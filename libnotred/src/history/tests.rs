use super::{HistoryFilter, HistoryStore};
use crate::model::Notification;
use crate::wire::{HistoryState, Urgency};

fn sample(id: u32, summary: &str) -> Notification {
    Notification {
        id,
        replaces_id: 0,
        app_id: "app".into(),
        summary: summary.into(),
        body: String::new(),
        urgency: Urgency::Normal,
        timeout_ms: -1,
        icon: None,
        action_keys: vec![],
        has_actions: false,
        timestamp: id as i64,
    }
}

fn temp_db(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("notred-hist-{name}-{}.db", std::process::id()))
}

#[test]
fn cap_five_drops_oldest() {
    let path = temp_db("cap");
    let store = HistoryStore::open(&path, true).unwrap();
    for i in 1..=6 {
        store.upsert_active(&sample(i, &format!("n{i}"))).unwrap();
        store.enforce_cap(5).unwrap();
    }
    let rows = store.list(&HistoryFilter::default()).unwrap();
    assert_eq!(rows.len(), 5);
    assert!(rows.iter().all(|r| r.id != 1));
    let _ = std::fs::remove_file(path);
}

#[test]
fn flush_wipes_on_open() {
    let path = temp_db("flush");
    {
        let store = HistoryStore::open(&path, false).unwrap();
        store.upsert_active(&sample(1, "a")).unwrap();
        assert_eq!(store.list(&HistoryFilter::default()).unwrap().len(), 1);
    }
    let store = HistoryStore::open(&path, true).unwrap();
    assert!(store.list(&HistoryFilter::default()).unwrap().is_empty());
    let _ = std::fs::remove_file(path);
}

#[test]
fn flush_false_keeps_rows() {
    let path = temp_db("keep");
    {
        let store = HistoryStore::open(&path, false).unwrap();
        store.upsert_active(&sample(1, "a")).unwrap();
    }
    let store = HistoryStore::open(&path, false).unwrap();
    assert_eq!(store.list(&HistoryFilter::default()).unwrap().len(), 1);
    let _ = std::fs::remove_file(path);
}

#[test]
fn mark_closed_and_remove() {
    let path = temp_db("state");
    let store = HistoryStore::open(&path, true).unwrap();
    store.upsert_active(&sample(1, "a")).unwrap();
    store.mark_closed(1).unwrap();
    let rows = store.list(&HistoryFilter::default()).unwrap();
    assert_eq!(rows[0].state, HistoryState::Closed);
    assert!(store.remove(1).unwrap());
    assert!(store.list(&HistoryFilter::default()).unwrap().is_empty());
    let _ = std::fs::remove_file(path);
}

#[test]
fn zero_cap_is_unlimited() {
    let path = temp_db("unlimited");
    let store = HistoryStore::open(&path, true).unwrap();
    for i in 1..=10 {
        store.upsert_active(&sample(i, "x")).unwrap();
        store.enforce_cap(0).unwrap();
    }
    assert_eq!(store.list(&HistoryFilter::default()).unwrap().len(), 10);
    let _ = std::fs::remove_file(path);
}

#[test]
fn active_only_filter() {
    let path = temp_db("filter");
    let store = HistoryStore::open(&path, true).unwrap();
    store.upsert_active(&sample(1, "a")).unwrap();
    store.upsert_active(&sample(2, "b")).unwrap();
    store.mark_closed(1).unwrap();
    let active = store
        .list(&HistoryFilter {
            active_only: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, 2);
    let _ = std::fs::remove_file(path);
}
