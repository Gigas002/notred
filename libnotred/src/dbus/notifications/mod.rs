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
        let icon = icon_from_image_data(&hints)
            .or_else(|| icon_from_hints(&hints))
            .or_else(|| icon_from_str(&app_icon));
        let has_actions = !actions.is_empty();
        let action_keys = actions
            .chunks(2)
            .filter_map(|c| c.first().cloned())
            .collect();
        let value = value_from_hints(&hints);
        let category = string_hint(&hints, "category");
        let desktop_entry = string_hint(&hints, "desktop-entry");
        let body_markup = self.state.runtime_config().await.body_markup;

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
            value,
            category,
            desktop_entry,
            body_markup,
        };

        let id = self.state.push_notification(notif).await;
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
    async fn get_capabilities(&self) -> Vec<String> {
        let mut caps = vec!["body".into(), "actions".into()];
        if self.state.runtime_config().await.body_markup {
            caps.push("body-markup".into());
        }
        caps
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

/// FDN `value` hint (INT32) — progress percent. Out-of-spec values (outside
/// `0..=100`) are treated as absent so subscribers never see an invalid bar.
fn value_from_hints(hints: &HashMap<String, OwnedValue>) -> Option<i32> {
    use std::ops::Deref;
    use zbus::zvariant::Value;

    hints
        .get("value")
        .and_then(|ov| {
            if let Value::I32(n) = ov.deref() {
                Some(*n)
            } else {
                None
            }
        })
        .filter(|n| (0..=100).contains(n))
}

/// String-valued hint lookup (`category`, `desktop-entry`, …).
fn string_hint(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    use std::ops::Deref;
    use zbus::zvariant::Value;

    hints.get(key).and_then(|ov| {
        if let Value::Str(s) = ov.deref() {
            Some(s.as_str().to_string())
        } else {
            None
        }
    })
}

/// Icon from the `image-data` hint (spec 1.2) / `image_data` / `icon_data`
/// (spec 1.1) — a raw pixel buffer `(iiibiiay)`: width, height, rowstride,
/// has_alpha, bits_per_sample, channels, data. Chat/messaging apps (Telegram,
/// Signal, …) use this to embed a per-notification avatar image that has no
/// icon-theme name or on-disk file to reference, so it's checked first.
fn icon_from_image_data(hints: &HashMap<String, OwnedValue>) -> Option<IconRef> {
    use std::ops::Deref;
    use zbus::zvariant::Value;

    let value = hints
        .get("image-data")
        .or_else(|| hints.get("image_data"))
        .or_else(|| hints.get("icon_data"))?;

    let Value::Structure(structure) = value.deref() else {
        return None;
    };
    let [
        Value::I32(width),
        Value::I32(height),
        Value::I32(rowstride),
        Value::Bool(has_alpha),
        Value::I32(bits_per_sample),
        Value::I32(channels),
        Value::Array(pixels),
    ] = structure.fields()
    else {
        return None;
    };

    let data: Vec<u8> = pixels
        .iter()
        .map(|v| match v {
            Value::U8(b) => Some(*b),
            _ => None,
        })
        .collect::<Option<_>>()?;

    if *width <= 0 || *height <= 0 || data.is_empty() {
        return None;
    }

    Some(IconRef::Raw {
        width: *width,
        height: *height,
        rowstride: *rowstride,
        has_alpha: *has_alpha,
        bits_per_sample: *bits_per_sample,
        channels: *channels,
        data,
    })
}

/// Icon from the `image-path` hint (spec 1.2) / `image_path` (spec 1.1,
/// still sent by some clients). Takes precedence over the legacy `app_icon`
/// positional argument, matching mako/dunst behavior — most real-world
/// senders (including `notify-send -i`) put the actual icon here, leaving
/// `app_icon` empty or set to an unrelated app-badge value.
fn icon_from_hints(hints: &HashMap<String, OwnedValue>) -> Option<IconRef> {
    use std::ops::Deref;
    use zbus::zvariant::Value;

    let raw = hints
        .get("image-path")
        .or_else(|| hints.get("image_path"))
        .and_then(|ov| {
            if let Value::Str(s) = ov.deref() {
                Some(s.as_str())
            } else {
                None
            }
        })?;

    let path = raw.strip_prefix("file://").unwrap_or(raw);
    icon_from_str(path)
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
