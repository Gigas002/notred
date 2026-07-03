use super::Queue;
use crate::model::{CloseReason, Notification};
use crate::wire::Urgency;

fn notif(app: &str, summary: &str) -> Notification {
    Notification {
        id: 0,
        replaces_id: 0,
        app_id: app.into(),
        summary: summary.into(),
        body: String::new(),
        urgency: Urgency::Normal,
        timeout_ms: -1,
        icon: None,
        action_keys: vec![],
        has_actions: false,
        timestamp: 0,
    }
}

#[tokio::test]
async fn push_assigns_sequential_ids() {
    let q = Queue::new();
    let id1 = q.push(notif("a", "one")).await;
    let id2 = q.push(notif("b", "two")).await;
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(q.len().await, 2);
}

#[tokio::test]
async fn push_replaces_existing_notification() {
    let q = Queue::new();
    let id = q.push(notif("a", "original")).await;

    let mut replacement = notif("a", "updated");
    replacement.replaces_id = id;
    let replaced_id = q.push(replacement).await;

    assert_eq!(replaced_id, id);
    assert_eq!(q.len().await, 1);
    let items = q.snapshot().await;
    assert_eq!(items[0].summary, "updated");
}

#[tokio::test]
async fn push_unknown_replaces_id_adds_new() {
    let q = Queue::new();
    let mut n = notif("a", "new");
    n.replaces_id = 999;
    let id = q.push(n).await;
    assert!(id > 0);
    assert_eq!(q.len().await, 1);
}

#[tokio::test]
async fn close_removes_and_broadcasts() {
    let q = Queue::new();
    let mut rx = q.subscribe_closes();
    let id = q.push(notif("a", "x")).await;

    let found = q.close(id, CloseReason::DismissedByUser).await;
    assert!(found);
    assert_eq!(q.len().await, 0);

    let ev = rx.try_recv().unwrap();
    assert_eq!(ev.id, id);
    assert_eq!(ev.reason, CloseReason::DismissedByUser);
}

#[tokio::test]
async fn close_unknown_returns_false() {
    let q = Queue::new();
    assert!(!q.close(999, CloseReason::DismissedByUser).await);
}

#[tokio::test]
async fn close_all_clears_queue() {
    let q = Queue::new();
    q.push(notif("a", "1")).await;
    q.push(notif("b", "2")).await;

    let ids = q.close_all(CloseReason::DismissedByUser).await;
    assert_eq!(ids.len(), 2);
    assert!(q.is_empty().await);
}

#[tokio::test]
async fn pause_holds_new_notifications() {
    let q = Queue::new();
    q.set_paused(true).await;
    q.push(notif("a", "held")).await;
    assert_eq!(q.len().await, 0);
    assert!(q.is_paused().await);
}

#[tokio::test]
async fn unpause_flushes_held_to_active() {
    let q = Queue::new();
    q.set_paused(true).await;
    q.push(notif("a", "held")).await;
    q.set_paused(false).await;
    assert_eq!(q.len().await, 1);
}

#[tokio::test]
async fn get_finds_active_and_held() {
    let q = Queue::new();
    let id = q.push(notif("a", "active")).await;
    assert!(q.get(id).await.is_some());

    q.set_paused(true).await;
    let held_id = q.push(notif("a", "held")).await;
    assert!(q.get(held_id).await.is_some());
}

#[tokio::test]
async fn max_visible_evicts_oldest() {
    let q = Queue::new();
    q.set_max_visible(2).await;
    q.push(notif("a", "1")).await;
    q.push(notif("b", "2")).await;
    let mut close_rx = q.subscribe_closes();
    q.push(notif("c", "3")).await;

    assert_eq!(q.len().await, 2);
    let items = q.snapshot().await;
    assert_eq!(items[0].summary, "2");
    assert_eq!(items[1].summary, "3");

    let ev = close_rx.try_recv().unwrap();
    assert_eq!(ev.reason, CloseReason::Undefined);
}

#[tokio::test]
async fn change_broadcast_fires_on_push_and_close() {
    let q = Queue::new();
    let mut rx = q.subscribe_changes();

    let id = q.push(notif("a", "x")).await;
    rx.try_recv().expect("change on push");

    q.close(id, CloseReason::DismissedByUser).await;
    rx.try_recv().expect("change on close");
}
