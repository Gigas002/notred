//! Tokio subprocess hooks for `[events]` config.

use crate::host::state::{ActivateEvent, RuntimeConfig};

/// Spawn the configured `on_action` hook without blocking the caller.
pub fn spawn_on_action(config: &RuntimeConfig, ev: &ActivateEvent) {
    let Some(argv) = config.on_action.clone() else {
        return;
    };
    if argv.is_empty() {
        return;
    }

    let id = ev.id.to_string();
    let key = ev.key.clone();
    let app_id = ev.app_id.clone();

    tokio::spawn(async move {
        let mut cmd = tokio::process::Command::new(&argv[0]);
        if argv.len() > 1 {
            cmd.args(&argv[1..]);
        }
        cmd.env("NOTRED_ID", &id)
            .env("NOTRED_ACTION_KEY", &key)
            .env("NOTRED_APP_ID", &app_id);

        match cmd.spawn() {
            Ok(mut child) => {
                if let Err(e) = child.wait().await {
                    tracing::warn!(%e, "on_action hook wait failed");
                }
            }
            Err(e) => tracing::warn!(%e, "on_action hook spawn failed"),
        }
    });
}

#[cfg(test)]
mod tests;
