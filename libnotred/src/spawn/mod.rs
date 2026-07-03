//! Tokio subprocess hooks for `[events]` config.

use std::collections::HashMap;

use crate::events::{EventKind, EventsHooks};
use crate::host::state::{ActivateEvent, RuntimeConfig};
use crate::model::Notification;

fn spawn_hook(argv: Option<Vec<String>>, env: HashMap<String, String>) {
    let Some(argv) = argv else {
        return;
    };
    if argv.is_empty() {
        return;
    }

    tokio::spawn(async move {
        let mut cmd = tokio::process::Command::new(&argv[0]);
        if argv.len() > 1 {
            cmd.args(&argv[1..]);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }

        match cmd.spawn() {
            Ok(mut child) => {
                if let Err(e) = child.wait().await {
                    tracing::warn!(%e, "event hook wait failed");
                }
            }
            Err(e) => tracing::warn!(%e, "event hook spawn failed"),
        }
    });
}

/// Spawn the resolved `on_action` hook without blocking the caller.
pub fn spawn_on_action(config: &RuntimeConfig, ev: &ActivateEvent) {
    let hooks = config.events.resolve(&ev.app_id, ev.urgency);
    let mut env = HashMap::new();
    env.insert("NOTRED_ID".into(), ev.id.to_string());
    env.insert("NOTRED_ACTION_KEY".into(), ev.key.clone());
    env.insert("NOTRED_APP_ID".into(), ev.app_id.clone());
    env.insert("NOTRED_EVENT_KIND".into(), "action".into());
    spawn_hook(hooks.on_action.clone(), env);
}

/// Spawn the resolved `on_notify` hook for a newly queued notification.
pub fn spawn_on_notify(hooks: &EventsHooks, notif: &Notification) {
    let mut env = HashMap::new();
    env.insert("NOTRED_ID".into(), notif.id.to_string());
    env.insert("NOTRED_APP_ID".into(), notif.app_id.clone());
    env.insert("NOTRED_SUMMARY".into(), notif.summary.clone());
    env.insert("NOTRED_EVENT_KIND".into(), "notify".into());
    spawn_hook(hooks.on_notify.clone(), env);
}

/// Spawn a resolved input hook (`on_button_*`, `on_touch`).
pub fn spawn_on_input(argv: &[String], notif: &Notification, kind: EventKind) {
    let mut env = HashMap::new();
    env.insert("NOTRED_ID".into(), notif.id.to_string());
    env.insert("NOTRED_APP_ID".into(), notif.app_id.clone());
    env.insert("NOTRED_SUMMARY".into(), notif.summary.clone());
    env.insert("NOTRED_EVENT_KIND".into(), kind.as_str().into());
    spawn_hook(Some(argv.to_vec()), env);
}

#[cfg(test)]
mod tests;
