//! `org.freedesktop.Notifications` zbus interface.

use std::collections::HashMap;
use std::sync::Arc;

use zbus::interface;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedValue;

use crate::host::state::HostState;
use crate::model::{CloseReason, Notification};
use crate::wire::{IconRef, Urgency};

pub const BUS_NAME: &str = "org.freedesktop.Notifications";
pub const OBJECT_PATH: &str = "/org/freedesktop/Notifications";

pub struct Notifications {
    state: Arc<HostState>,
}

impl Notifications {
    pub fn new(state: Arc<HostState>) -> Self {
        Self { state }
    }
}

#[interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    /// FDN `Notify` — add or replace a notification; returns the assigned id.
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let urgency = urgency_from_hints(&hints);
        let icon = icon_from_str(&app_icon);
        let has_actions = !actions.is_empty();
        let action_keys = actions
            .chunks(2)
            .filter_map(|c| c.first().cloned())
            .collect();

        let notif = Notification {
            id: 0,
            replaces_id,
            app_id: app_name,
            summary,
            body,
            urgency,
            timeout_ms: expire_timeout,
            icon,
            action_keys,
            has_actions,
            timestamp: now_unix(),
        };

        let id = self.state.push_notification(notif).await;
        #[cfg(feature = "history")]
        {
            let mut stored = notif;
            stored.id = id;
            self.state.record_notify(&stored).await;
        }
        tracing::debug!(id, "FDN Notify");
        id
    }

    /// FDN `CloseNotification` — close a notification by id.
    async fn close_notification(
        &self,
        id: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        if self.state.queue.close(id, CloseReason::ClosedByCall).await {
            tracing::debug!(id, "FDN CloseNotification");
            if let Err(e) =
                Self::notification_closed(&emitter, id, CloseReason::ClosedByCall.into()).await
            {
                tracing::warn!(%e, id, "failed to emit NotificationClosed signal");
            }
        }
    }

    /// FDN `GetCapabilities`.
    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".into(), "actions".into()]
    }

    /// FDN `GetServerInformation`.
    fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("notred", "notred", env!("CARGO_PKG_VERSION"), "1.2")
    }

    /// Signal: `NotificationClosed(id, reason)`.
    #[zbus(signal)]
    pub async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    /// Signal: `ActionInvoked(id, action_key)`.
    #[zbus(signal)]
    pub async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;
}

fn urgency_from_hints(hints: &HashMap<String, OwnedValue>) -> Urgency {
    use std::ops::Deref;
    use zbus::zvariant::Value;

    hints
        .get("urgency")
        .and_then(|ov| {
            if let Value::U8(n) = ov.deref() {
                Some(*n)
            } else {
                None
            }
        })
        .map(|n| match n {
            0 => Urgency::Low,
            2 => Urgency::Critical,
            _ => Urgency::Normal,
        })
        .unwrap_or(Urgency::Normal)
}

fn icon_from_str(s: &str) -> Option<IconRef> {
    if s.is_empty() {
        None
    } else if s.contains('/') {
        Some(IconRef::Path { path: s.into() })
    } else {
        Some(IconRef::Name { name: s.into() })
    }
}

fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
