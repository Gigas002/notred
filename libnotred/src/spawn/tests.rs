use super::spawn_on_action;
use crate::host::state::{ActivateEvent, RuntimeConfig};

use crate::wire::Urgency;

#[test]
fn spawn_on_action_no_hook_is_noop() {
    let cfg = RuntimeConfig::default();
    let ev = ActivateEvent {
        id: 1,
        key: "default".into(),
        app_id: "app".into(),
        urgency: Urgency::Normal,
    };
    spawn_on_action(&cfg, &ev);
}
