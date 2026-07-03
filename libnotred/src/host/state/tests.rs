#[cfg(feature = "history")]
mod host_history_tests {
    use std::sync::Arc;

    use crate::history::{HistoryFilter, HistoryStore};
    use crate::host::state::{HistoryError, HostState, RuntimeConfig};
    use crate::model::Notification;
    use crate::queue::Queue;
    use crate::wire::Urgency;

    fn sample(id: u32) -> Notification {
        Notification {
            id,
            replaces_id: 0,
            app_id: "app".into(),
            summary: "s".into(),
            body: String::new(),
            urgency: Urgency::Normal,
            timeout_ms: -1,
            icon: None,
            action_keys: vec![],
            has_actions: false,
            timestamp: 1,
        }
    }

    fn temp_db() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("notred-host-hist-{}.db", std::process::id()))
    }

    #[tokio::test]
    async fn disabled_history_skips_writes() {
        let queue = Arc::new(Queue::new());
        let mut runtime = RuntimeConfig::default();
        runtime.history.enabled = false;
        let state = HostState::new(runtime, queue);
        state.record_notify(&sample(1)).await;
        assert!(matches!(
            state.list_history(HistoryFilter::default()).await,
            Err(HistoryError::Disabled)
        ));
    }

    #[tokio::test]
    async fn record_and_list_history() {
        let path = temp_db();
        let queue = Arc::new(Queue::new());
        let mut runtime = RuntimeConfig::default();
        runtime.history.enabled = true;
        let state = HostState::new(runtime.clone(), Arc::clone(&queue));
        let store = Arc::new(HistoryStore::open(&path, true).unwrap());
        state.init_history(store, &runtime.history).await;
        state.record_notify(&sample(1)).await;
        let rows = state.list_history(HistoryFilter::default()).await.unwrap();
        assert_eq!(rows.len(), 1);
        let _ = std::fs::remove_file(path);
    }
}

mod input_tests {
    use std::sync::Arc;

    use crate::events::{EventKind, EventsHooks};
    use crate::host::state::{HostState, InputError, RuntimeConfig};
    use crate::model::Notification;
    use crate::queue::Queue;
    use crate::wire::Urgency;

    fn notif(has_actions: bool) -> Notification {
        Notification {
            id: 0,
            replaces_id: 0,
            app_id: "app".into(),
            summary: "hi".into(),
            body: String::new(),
            urgency: Urgency::Normal,
            timeout_ms: -1,
            icon: None,
            action_keys: if has_actions {
                vec!["default".into()]
            } else {
                vec![]
            },
            has_actions,
            timestamp: 0,
        }
    }

    #[tokio::test]
    async fn invalid_event_kind_rejected() {
        let queue = Arc::new(Queue::new());
        let state = HostState::new(RuntimeConfig::default(), Arc::clone(&queue));
        queue.push(notif(false)).await;
        assert!(matches!(
            state.handle_input(1, "left_button_click").await,
            Err(InputError::InvalidEventKind { .. })
        ));
    }

    #[tokio::test]
    async fn button_left_dismisses_without_actions() {
        let queue = Arc::new(Queue::new());
        let state = HostState::new(RuntimeConfig::default(), Arc::clone(&queue));
        let id = queue.push(notif(false)).await;
        state.handle_input(id, "button_left").await.unwrap();
        assert!(queue.is_empty().await);
    }

    #[tokio::test]
    async fn button_right_dismisses_by_default() {
        let queue = Arc::new(Queue::new());
        let state = HostState::new(RuntimeConfig::default(), Arc::clone(&queue));
        let id = queue.push(notif(true)).await;
        state.handle_input(id, "button_right").await.unwrap();
        assert!(queue.is_empty().await);
    }

    #[tokio::test]
    async fn configured_hook_prevents_default_dismiss() {
        let queue = Arc::new(Queue::new());
        let mut runtime = RuntimeConfig::default();
        runtime.events.base.on_button_left = Some(vec!["true".into()]);
        let state = HostState::new(runtime, Arc::clone(&queue));
        let id = queue.push(notif(false)).await;
        state.handle_input(id, "button_left").await.unwrap();
        assert_eq!(queue.len().await, 1);
    }

    #[tokio::test]
    async fn button_left_with_actions_emits_activate() {
        let queue = Arc::new(Queue::new());
        let state = HostState::new(RuntimeConfig::default(), Arc::clone(&queue));
        let mut rx = state.subscribe_activates();
        let id = queue.push(notif(true)).await;
        state.handle_input(id, "button_left").await.unwrap();
        let ev = rx.try_recv().unwrap();
        assert_eq!(ev.id, id);
        assert_eq!(ev.key, "default");
    }

    #[test]
    fn event_kind_parse_and_hook() {
        assert_eq!(EventKind::parse("button_left"), Some(EventKind::ButtonLeft));
        assert!(EventKind::parse("nope").is_none());
        let hooks = EventsHooks {
            on_touch: Some(vec!["echo".into()]),
            ..Default::default()
        };
        assert!(EventKind::Touch.hook(&hooks).is_some());
        assert!(EventKind::ButtonLeft.hook(&hooks).is_none());
    }
}
