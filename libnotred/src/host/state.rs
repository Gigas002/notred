//! Shared daemon runtime state (queue, pause, reloadable config).

use std::sync::Arc;

use tokio::sync::{RwLock, broadcast};

#[cfg(feature = "history")]
use crate::history::{HistoryFilter, HistoryStore};
#[cfg(feature = "history")]
use crate::model::Notification;
#[cfg(feature = "history")]
use crate::wire::HistoryRow;

/// Runtime policy loaded from `notred.toml` (plain structs — no TOML in the library).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// Optional argv for an action-activation hook (`[events].on_action`).
    pub on_action: Option<Vec<String>>,
    #[cfg(feature = "history")]
    pub history: HistorySettings,
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
            enabled: true,
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
}

/// Errors from [`HostState::activate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivateError {
    NotFound,
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
        let (activate_tx, _) = broadcast::channel(64);
        let (reload_tx, _) = broadcast::channel(16);
        #[cfg(feature = "history")]
        let (history_changed_tx, _) = broadcast::channel(64);
        Arc::new(Self {
            queue,
            config: RwLock::new(runtime),
            activate_tx,
            reload_tx,
            #[cfg(feature = "history")]
            history: RwLock::new(None),
            #[cfg(feature = "history")]
            history_changed_tx,
        })
    }

    pub async fn runtime_config(&self) -> RuntimeConfig {
        self.config.read().await.clone()
    }

    pub async fn apply_config(&self, cfg: RuntimeConfig) {
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
    pub async fn list_history(&self, filter: HistoryFilter) -> Result<Vec<HistoryRow>, HistoryError> {
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
        let notif = self
            .queue
            .get(id)
            .await
            .ok_or(ActivateError::NotFound)?;

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
        };
        let _ = self.activate_tx.send(ev);
        Ok(())
    }
}
