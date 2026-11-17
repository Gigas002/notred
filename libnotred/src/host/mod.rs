//! `NotredHost` — owns the D-Bus connection, queue, and IPC server.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::broadcast;
use zbus::connection;

use crate::dbus::notifications::{self, Notifications};
#[cfg(feature = "history")]
use crate::history::HistoryStore;
use crate::host::state::{ActivateEvent, HostState, RuntimeConfig};
use crate::ipc::IpcError;
use crate::ipc::server::Server;
use crate::model::CloseReason;
use crate::queue::Queue;
use crate::spawn::spawn_on_action;

pub mod state;

/// Configuration passed from the `notred` binary into the host.
#[derive(Clone)]
pub struct HostConfig {
    pub socket_path: PathBuf,
    pub runtime: RuntimeConfig,
    #[cfg(feature = "history")]
    pub history_path: PathBuf,
    /// Reload runtime policy from disk (returns updated [`RuntimeConfig`]).
    pub reload: Option<Arc<dyn Fn() -> Result<RuntimeConfig, String> + Send + Sync>>,
}

/// Errors from the host runtime.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error(transparent)]
    Ipc(#[from] IpcError),

    #[error("D-Bus error: {0}")]
    Dbus(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<zbus::Error> for HostError {
    fn from(e: zbus::Error) -> Self {
        Self::Dbus(e.to_string())
    }
}

impl From<zbus::fdo::Error> for HostError {
    fn from(e: zbus::fdo::Error) -> Self {
        Self::Dbus(e.to_string())
    }
}

/// The daemon host: FDN D-Bus + queue + IPC server.
pub struct NotredHost {
    config: HostConfig,
}

impl NotredHost {
    pub fn new(config: HostConfig) -> Self {
        Self { config }
    }

    /// Run the host until an error.
    pub async fn run(&self) -> Result<(), HostError> {
        self.run_until(std::future::pending()).await
    }

    /// Run the host until an error or until `shutdown` completes; then remove
    /// the IPC socket and return `Ok`.
    pub async fn run_until(
        &self,
        shutdown: impl std::future::Future<Output = ()>,
    ) -> Result<(), HostError> {
        let queue = Arc::new(Queue::new());
        queue.set_max_visible(self.config.runtime.max_visible).await;
        let state = HostState::new(self.config.runtime.clone(), Arc::clone(&queue));

        #[cfg(feature = "history")]
        if self.config.runtime.history.enabled {
            match HistoryStore::open(&self.config.history_path, self.config.runtime.history.flush) {
                Ok(store) => {
                    // Ids restart at 1 each run; a kept history must not get
                    // its rows overwritten by new notifications reusing them.
                    match store.max_id() {
                        Ok(max) => queue.allocate_ids_after(max).await,
                        Err(e) => tracing::warn!(%e, "can't read history ids"),
                    }
                    state
                        .init_history(Arc::new(store), &self.config.runtime.history)
                        .await;
                    tracing::info!(
                        path = %self.config.history_path.display(),
                        "history database open"
                    );
                }
                Err(e) => tracing::error!(%e, "failed to open history database"),
            }
        }

        let close_rx = queue.subscribe_closes();
        let activate_rx = state.subscribe_activates();

        #[cfg(feature = "history")]
        {
            let history_state = Arc::clone(&state);
            let mut history_close_rx = queue.subscribe_closes();
            tokio::spawn(async move {
                loop {
                    match history_close_rx.recv().await {
                        Ok(ev) => history_state.mark_history_closed(ev.id).await,
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
        }

        let conn = connection::Builder::session()
            .name(notifications::BUS_NAME)
            .serve_at(
                notifications::OBJECT_PATH,
                Notifications::new(Arc::clone(&state)),
            )
            .build()
            .await?;

        tracing::info!("D-Bus name acquired: {}", notifications::BUS_NAME);

        let signal_conn = conn.clone();
        let signal_state = Arc::clone(&state);
        let signal_task = tokio::spawn(async move {
            emit_signals_task(signal_conn, close_rx, activate_rx, signal_state).await;
        });

        let server = Server::new(&self.config.socket_path, state, self.config.reload.clone());

        let result = tokio::select! {
            result = server.run() => result.map_err(HostError::from),
            _ = signal_task => Ok(()),
            () = shutdown => {
                tracing::info!("shutting down");
                Ok(())
            }
        };
        let _ = std::fs::remove_file(&self.config.socket_path);
        result
    }
}

/// Background task: emit D-Bus signals and run `[events]` hooks.
async fn emit_signals_task(
    conn: zbus::Connection,
    mut close_rx: broadcast::Receiver<crate::queue::ClosedEvent>,
    mut activate_rx: broadcast::Receiver<ActivateEvent>,
    state: Arc<HostState>,
) {
    let emitter = match zbus::object_server::SignalEmitter::new(&conn, notifications::OBJECT_PATH) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!(%e, "failed to create signal emitter");
            return;
        }
    };

    loop {
        tokio::select! {
            close = close_rx.recv() => {
                match close {
                    Ok(ev) => {
                        if ev.reason == CloseReason::ClosedByCall {
                            continue;
                        }
                        if let Err(e) = Notifications::notification_closed(
                            &emitter,
                            ev.id,
                            u32::from(ev.reason),
                        )
                        .await
                        {
                            tracing::warn!(%e, id = ev.id, "failed to emit NotificationClosed");
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(n, "close signal task lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            activate = activate_rx.recv() => {
                match activate {
                    Ok(ev) => {
                        let config = state.runtime_config().await;
                        spawn_on_action(&config, &ev);
                        if let Err(e) = Notifications::action_invoked(
                            &emitter,
                            ev.id,
                            &ev.key,
                        )
                        .await
                        {
                            tracing::warn!(%e, id = ev.id, "failed to emit ActionInvoked");
                        }
                        // The spec: an invoked action removes the notification
                        // unless it set the `resident` hint. After ActionInvoked,
                        // so the sender sees the action before the close.
                        if !ev.resident {
                            state.queue.close(ev.id, CloseReason::DismissedByUser).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(n, "activate signal task lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}
