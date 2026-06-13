//! `NotredHost` — owns the D-Bus connection, queue, and IPC server.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::broadcast;
use zbus::connection;

use crate::dbus::notifications::{self, Notifications};
use crate::ipc::IpcError;
use crate::ipc::server::Server;
use crate::model::CloseReason;
use crate::queue::{ClosedEvent, Queue};

/// Configuration passed from the `notred` binary into the host.
#[derive(Debug, Clone)]
pub struct HostConfig {
    pub socket_path: PathBuf,
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

    /// Run the host until an error or signal.
    pub async fn run(&self) -> Result<(), HostError> {
        let queue = Arc::new(Queue::new());

        let close_rx = queue.subscribe_closes();

        let conn = connection::Builder::session()?
            .name(notifications::BUS_NAME)?
            .serve_at(
                notifications::OBJECT_PATH,
                Notifications::new(Arc::clone(&queue)),
            )?
            .build()
            .await?;

        tracing::info!("D-Bus name acquired: {}", notifications::BUS_NAME);

        let signal_conn = conn.clone();
        let signal_task = tokio::spawn(async move {
            emit_signals_task(signal_conn, close_rx).await;
        });

        let server = Server::new(&self.config.socket_path, Arc::clone(&queue));

        tokio::select! {
            result = server.run() => {
                result?;
            }
            _ = signal_task => {}
        }

        Ok(())
    }
}

/// Background task: emit `NotificationClosed` D-Bus signals from the close channel.
async fn emit_signals_task(conn: zbus::Connection, mut rx: broadcast::Receiver<ClosedEvent>) {
    let emitter = match zbus::object_server::SignalEmitter::new(&conn, notifications::OBJECT_PATH) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!(%e, "failed to create signal emitter");
            return;
        }
    };

    loop {
        let ev = match rx.recv().await {
            Ok(ev) => ev,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(
                    n,
                    "signal task lagged; some NotificationClosed signals may be lost"
                );
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => break,
        };

        // FDN CloseNotification already emits its own signal inline (to avoid
        // duplicate emission here). Skip ClosedByCall to avoid double-emit.
        if ev.reason == CloseReason::ClosedByCall {
            continue;
        }

        if let Err(e) =
            Notifications::notification_closed(&emitter, ev.id, u32::from(ev.reason)).await
        {
            tracing::warn!(%e, id = ev.id, "failed to emit NotificationClosed signal");
        }
    }
}

#[cfg(test)]
mod tests;
