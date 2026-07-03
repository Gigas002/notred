use std::sync::Arc;
use std::time::Duration;

use super::{TimeoutManager, effective_timeout_ms};
use crate::model::{CloseReason, Notification};
use crate::queue::Queue;
use crate::wire::Urgency;

fn notif() -> Notification {
    Notification {
        id: 0,
        replaces_id: 0,
        app_id: "a".into(),
        summary: "x".into(),
        body: String::new(),
        urgency: Urgency::Normal,
        timeout_ms: 30,
        icon: None,
        action_keys: vec![],
        has_actions: false,
        timestamp: 0,
    }
}

#[tokio::test]
async fn timer_closes_notification() {
    let queue = Arc::new(Queue::new());
    let id = queue.push(notif()).await;
    let mgr = Arc::new(TimeoutManager::new(Arc::clone(&queue)));
    mgr.clone().spawn_cancel_task(queue.subscribe_closes());
    mgr.schedule(id, 30, 0).await;
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(queue.is_empty().await);
}

#[tokio::test]
async fn early_close_cancels_timer() {
    let queue = Arc::new(Queue::new());
    let id = queue.push(notif()).await;
    let mgr = Arc::new(TimeoutManager::new(Arc::clone(&queue)));
    mgr.clone().spawn_cancel_task(queue.subscribe_closes());
    mgr.schedule(id, 500, 0).await;
    queue.close(id, CloseReason::DismissedByUser).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(queue.is_empty().await);
}

#[test]
fn zero_means_persistent() {
    assert_eq!(effective_timeout_ms(0, 5000), None);
}

#[test]
fn positive_uses_fdn_value() {
    assert_eq!(effective_timeout_ms(3000, 5000), Some(3000));
}

#[test]
fn negative_uses_default_when_set() {
    assert_eq!(effective_timeout_ms(-1, 5000), Some(5000));
    assert_eq!(effective_timeout_ms(-1, 0), None);
}
