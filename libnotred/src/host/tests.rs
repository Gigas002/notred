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
        let state = HostState::new(RuntimeConfig::default(), Arc::clone(&queue));
        let store = Arc::new(HistoryStore::open(&path, true).unwrap());
        state
            .init_history(store, &RuntimeConfig::default().history)
            .await;
        state.record_notify(&sample(1)).await;
        let rows = state.list_history(HistoryFilter::default()).await.unwrap();
        assert_eq!(rows.len(), 1);
        let _ = std::fs::remove_file(path);
    }
}
