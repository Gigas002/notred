//! Shared daemon runtime state (queue, pause, reloadable config).

use std::sync::Arc;

use tokio::sync::{RwLock, broadcast};

/// Runtime policy loaded from `notred.toml` (plain structs — no TOML in the library).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// Optional argv for an action-activation hook (`[events].on_action`).
    pub on_action: Option<Vec<String>>,
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

/// Shared state for D-Bus, IPC, and background tasks.
pub struct HostState {
    pub queue: Arc<crate::queue::Queue>,
    config: RwLock<RuntimeConfig>,
    activate_tx: broadcast::Sender<ActivateEvent>,
    reload_tx: broadcast::Sender<()>,
}

impl HostState {
    pub fn new(runtime: RuntimeConfig, queue: Arc<crate::queue::Queue>) -> Arc<Self> {
        let (activate_tx, _) = broadcast::channel(64);
        let (reload_tx, _) = broadcast::channel(16);
        Arc::new(Self {
            queue,
            config: RwLock::new(runtime),
            activate_tx,
            reload_tx,
        })
    }

    pub async fn runtime_config(&self) -> RuntimeConfig {
        self.config.read().await.clone()
    }

    pub async fn apply_config(&self, cfg: RuntimeConfig) {
        *self.config.write().await = cfg;
        let _ = self.reload_tx.send(());
    }

    pub fn subscribe_activates(&self) -> broadcast::Receiver<ActivateEvent> {
        self.activate_tx.subscribe()
    }

    pub fn subscribe_reload(&self) -> broadcast::Receiver<()> {
        self.reload_tx.subscribe()
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
