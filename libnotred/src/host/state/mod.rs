//! Shared daemon runtime state (queue, pause, reloadable config).

use std::sync::Arc;

use tokio::sync::{RwLock, broadcast};

use crate::events::{EventKind, EventsPolicy};
#[cfg(feature = "history")]
use crate::history::{HistoryFilter, HistoryStore};
use crate::model::Notification;
use crate::timeouts::TimeoutManager;
#[cfg(feature = "history")]
use crate::wire::HistoryRow;

/// Runtime policy loaded from `notred.toml` (plain structs — no TOML in the library).
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Active queue cap (`0` = unlimited).
    pub max_visible: u32,
    /// Default auto-dismiss when FDN `expire_timeout` is `-1` (`0` = persistent).
    pub default_timeout_ms: u32,
    pub events: EventsPolicy,
    #[cfg(feature = "history")]
    pub history: HistorySettings,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_visible: 0,
            default_timeout_ms: 0,
            events: EventsPolicy {
                base: Default::default(),
                overrides: vec![],
            },
            #[cfg(feature = "history")]
            history: HistorySettings::default(),
        }
    }
}

/// `[history]` section (`history` feature).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistorySettings {
    pub enabled: bool,
    /// Wipe DB on daemon start only (not on `reload`).
    pub flush: bool,
    /// `0` = unlimited; default `5`.
    pub max_entries: u32,
}

impl Default for HistorySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            flush: true,
            max_entries: 5,
        }
    }
}

/// Broadcast when a subscriber should run an action hook + D-Bus `ActionInvoked`.
#[derive(Debug, Clone)]
pub struct ActivateEvent {
    pub id: u32,
    pub key: String,
    pub app_id: String,
    pub urgency: crate::wire::Urgency,
}

/// Errors from [`HostState::activate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivateError {
    NotFound,
    InvalidActionKey { key: String },
}

/// Errors from [`HostState::handle_input`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputError {
    NotFound,
    InvalidEventKind { kind: String },
    InvalidActionKey { key: String },
}

#[cfg(feature = "history")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryError {
    Disabled,
    NotFound,
}

/// Shared state for D-Bus, IPC, and background tasks.
pub struct HostState {
    pub queue: Arc<crate::queue::Queue>,
    pub timeouts: Arc<TimeoutManager>,
    config: RwLock<RuntimeConfig>,
    activate_tx: broadcast::Sender<ActivateEvent>,
    reload_tx: broadcast::Sender<()>,
    #[cfg(feature = "history")]
    history: RwLock<Option<Arc<HistoryStore>>>,
    #[cfg(feature = "history")]
    history_changed_tx: broadcast::Sender<()>,
}

impl HostState {
    pub fn new(runtime: RuntimeConfig, queue: Arc<crate::queue::Queue>) -> Arc<Self> {
        let timeouts = Arc::new(TimeoutManager::new(Arc::clone(&queue)));
        let (activate_tx, _) = broadcast::channel(64);
        let (reload_tx, _) = broadcast::channel(16);
        #[cfg(feature = "history")]
        let (history_changed_tx, _) = broadcast::channel(64);
        let state = Arc::new(Self {
            queue: Arc::clone(&queue),
            timeouts,
            config: RwLock::new(runtime),
            activate_tx,
            reload_tx,
            #[cfg(feature = "history")]
            history: RwLock::new(None),
            #[cfg(feature = "history")]
            history_changed_tx,
        });
        let cancel_timeouts = Arc::clone(&state.timeouts);
        let close_rx = queue.subscribe_closes();
        cancel_timeouts.spawn_cancel_task(close_rx);
        state
    }

    pub async fn runtime_config(&self) -> RuntimeConfig {
        self.config.read().await.clone()
    }

    pub async fn apply_config(&self, cfg: RuntimeConfig) {
        self.queue.set_max_visible(cfg.max_visible).await;
        #[cfg(feature = "history")]
        {
            let old = self.config.read().await.history.clone();
            let new = cfg.history.clone();
            if new.enabled
                && old.max_entries != new.max_entries
                && let Some(store) = self.history.read().await.clone()
            {
                let _ = store.enforce_cap(new.max_entries);
            }
        }
        *self.config.write().await = cfg;
        let _ = self.reload_tx.send(());
    }

    /// Push to the active queue, schedule expiry, and run `on_notify` hook if configured.
    pub async fn push_notification(&self, mut notif: Notification) -> u32 {
        let cfg = self.config.read().await.clone();
        let id = self.queue.push(notif.clone()).await;
        notif.id = id;
        self.timeouts
            .schedule(id, notif.timeout_ms, cfg.default_timeout_ms)
            .await;
        let hooks = cfg.events.resolve(&notif.app_id, notif.urgency);
        crate::spawn::spawn_on_notify(&hooks, &notif);
        #[cfg(feature = "history")]
        self.record_notify(&notif).await;
        id
    }

    pub fn subscribe_activates(&self) -> broadcast::Receiver<ActivateEvent> {
        self.activate_tx.subscribe()
    }

    pub fn subscribe_reload(&self) -> broadcast::Receiver<()> {
        self.reload_tx.subscribe()
    }

    #[cfg(feature = "history")]
    pub fn subscribe_history_changes(&self) -> broadcast::Receiver<()> {
        self.history_changed_tx.subscribe()
    }

    #[cfg(feature = "history")]
    pub async fn init_history(&self, store: Arc<HistoryStore>, settings: &HistorySettings) {
        *self.history.write().await = Some(store);
        let mut cfg = self.config.write().await;
        cfg.history = settings.clone();
    }

    #[cfg(feature = "history")]
    async fn history_store(&self) -> Result<Arc<HistoryStore>, HistoryError> {
        let cfg = self.config.read().await;
        if !cfg.history.enabled {
            return Err(HistoryError::Disabled);
        }
        self.history
            .read()
            .await
            .clone()
            .ok_or(HistoryError::Disabled)
    }

    #[cfg(feature = "history")]
    fn emit_history_changed(&self) {
        let _ = self.history_changed_tx.send(());
    }

    #[cfg(feature = "history")]
    pub async fn record_notify(&self, notif: &Notification) {
        let Ok(store) = self.history_store().await else {
            return;
        };
        let max = self.config.read().await.history.max_entries;
        if store.upsert_active(notif).is_ok() {
            let _ = store.enforce_cap(max);
            self.emit_history_changed();
        }
    }

    #[cfg(feature = "history")]
    pub async fn mark_history_closed(&self, id: u32) {
        let Ok(store) = self.history_store().await else {
            return;
        };
        if store.mark_closed(id).is_ok() {
            self.emit_history_changed();
        }
    }

    #[cfg(feature = "history")]
    pub async fn list_history(
        &self,
        filter: HistoryFilter,
    ) -> Result<Vec<HistoryRow>, HistoryError> {
        let store = self.history_store().await?;
        store.list(&filter).map_err(|_| HistoryError::Disabled)
    }

    #[cfg(feature = "history")]
    pub async fn remove_history(&self, id: u32) -> Result<(), HistoryError> {
        let store = self.history_store().await?;
        if !store.remove(id).map_err(|_| HistoryError::Disabled)? {
            return Err(HistoryError::NotFound);
        }
        self.queue
            .close(id, crate::model::CloseReason::DismissedByUser)
            .await;
        self.emit_history_changed();
        Ok(())
    }

    /// Resolve action key and enqueue activation (D-Bus signal + optional shell).
    pub async fn activate(&self, id: u32, key: Option<String>) -> Result<(), ActivateError> {
        let key = key.unwrap_or_else(|| "default".into());
        let notif = self.queue.get(id).await.ok_or(ActivateError::NotFound)?;

        if !notif.has_actions && key != "default" {
            return Err(ActivateError::InvalidActionKey { key });
        }

        if notif.has_actions && !notif.action_keys.contains(&key) {
            return Err(ActivateError::InvalidActionKey { key });
        }

        let ev = ActivateEvent {
            id,
            key,
            app_id: notif.app_id,
            urgency: notif.urgency,
        };
        let _ = self.activate_tx.send(ev);
        Ok(())
    }

    /// Handle a subscriber pointer gesture: run merged hook or default policy.
    pub async fn handle_input(&self, id: u32, event_kind: &str) -> Result<(), InputError> {
        let kind = EventKind::parse(event_kind).ok_or_else(|| InputError::InvalidEventKind {
            kind: event_kind.to_string(),
        })?;

        let notif = self.queue.get(id).await.ok_or(InputError::NotFound)?;
        let cfg = self.config.read().await.clone();
        let hooks = cfg.events.resolve(&notif.app_id, notif.urgency);

        if let Some(argv) = kind.hook(&hooks) {
            crate::spawn::spawn_on_input(argv, &notif, kind);
            return Ok(());
        }

        match kind {
            EventKind::ButtonLeft | EventKind::Touch => {
                if notif.has_actions {
                    self.activate(id, None).await.map_err(|e| match e {
                        ActivateError::NotFound => InputError::NotFound,
                        ActivateError::InvalidActionKey { key } => {
                            InputError::InvalidActionKey { key }
                        }
                    })?;
                } else {
                    self.queue
                        .close(id, crate::model::CloseReason::DismissedByUser)
                        .await;
                }
            }
            EventKind::ButtonMiddle | EventKind::ButtonRight => {
                self.queue
                    .close(id, crate::model::CloseReason::DismissedByUser)
                    .await;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
