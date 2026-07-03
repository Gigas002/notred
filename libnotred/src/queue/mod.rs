//! Active notification queue with broadcast fan-out.

use tokio::sync::{RwLock, broadcast};

use crate::model::{CloseReason, Notification};
use crate::wire::MinimalNotification;

/// Broadcast capacity for queue-change and close-event channels.
const CHANNEL_CAP: usize = 64;

/// Broadcast payload for specific close events (drives D-Bus signals).
#[derive(Debug, Clone, Copy)]
pub struct ClosedEvent {
    pub id: u32,
    pub reason: CloseReason,
}

struct QueueInner {
    next_id: u32,
    items: Vec<Notification>,
    /// Notifications received while paused (not visible to subscribers).
    held: Vec<Notification>,
    paused: bool,
}

/// Thread-safe notification queue.
///
/// Subscribers call [`Queue::subscribe_changes`] / [`Queue::subscribe_closes`]
/// to receive broadcast notifications on mutation.
pub struct Queue {
    inner: RwLock<QueueInner>,
    /// Fires `()` on any queue mutation (add or remove).
    change_tx: broadcast::Sender<()>,
    /// Fires [`ClosedEvent`] when a notification is removed.
    close_tx: broadcast::Sender<ClosedEvent>,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    pub fn new() -> Self {
        let (change_tx, _) = broadcast::channel(CHANNEL_CAP);
        let (close_tx, _) = broadcast::channel(CHANNEL_CAP);
        Self {
            inner: RwLock::new(QueueInner {
                next_id: 1,
                items: vec![],
                held: vec![],
                paused: false,
            }),
            change_tx,
            close_tx,
        }
    }

    /// Subscribe to queue-change events (add or remove).
    pub fn subscribe_changes(&self) -> broadcast::Receiver<()> {
        self.change_tx.subscribe()
    }

    /// Subscribe to close events (one per closed notification).
    pub fn subscribe_closes(&self) -> broadcast::Receiver<ClosedEvent> {
        self.close_tx.subscribe()
    }

    /// Push a notification, handling `replaces_id`. Returns the assigned id.
    ///
    /// When paused, notifications are held internally and not broadcast until
    /// [`Queue::set_paused`](Self::set_paused)(`false`).
    pub async fn push(&self, mut notif: Notification) -> u32 {
        let mut inner = self.inner.write().await;

        let id = assign_id(&mut inner, &mut notif);

        let broadcast = if inner.paused {
            let changes_visible =
                notif.replaces_id > 0 && inner.items.iter().any(|n| n.id == notif.replaces_id);
            if changes_visible {
                insert_or_replace(&mut inner.items, notif);
            } else {
                insert_or_replace(&mut inner.held, notif);
            }
            changes_visible
        } else {
            if notif.replaces_id > 0 {
                remove_by_id(&mut inner.held, notif.replaces_id);
            }
            insert_or_replace(&mut inner.items, notif);
            true
        };

        drop(inner);
        if broadcast {
            let _ = self.change_tx.send(());
        }
        id
    }

    /// Whether new notifications are held instead of surfaced to subscribers.
    pub async fn is_paused(&self) -> bool {
        self.inner.read().await.paused
    }

    /// Pause or unpause ingestion. Unpausing moves held notifications active and
    /// broadcasts one change event.
    pub async fn set_paused(&self, paused: bool) {
        let mut inner = self.inner.write().await;
        if inner.paused == paused {
            return;
        }
        inner.paused = paused;
        if !paused && !inner.held.is_empty() {
            let held = std::mem::take(&mut inner.held);
            inner.items.extend(held);
            drop(inner);
            let _ = self.change_tx.send(());
        }
    }

    /// Lookup an active or held notification by id.
    pub async fn get(&self, id: u32) -> Option<Notification> {
        let inner = self.inner.read().await;
        find_by_id(&inner.items, id)
            .or_else(|| find_by_id(&inner.held, id))
            .cloned()
    }

    /// Close one notification. Returns `false` if the id was not found.
    pub async fn close(&self, id: u32, reason: CloseReason) -> bool {
        let mut inner = self.inner.write().await;
        if remove_by_id(&mut inner.items, id) || remove_by_id(&mut inner.held, id) {
            drop(inner);
            let _ = self.close_tx.send(ClosedEvent { id, reason });
            let _ = self.change_tx.send(());
            true
        } else {
            false
        }
    }

    /// Close all notifications. Returns the ids that were closed.
    pub async fn close_all(&self, reason: CloseReason) -> Vec<u32> {
        let mut inner = self.inner.write().await;
        let mut ids: Vec<u32> = inner.items.iter().map(|n| n.id).collect();
        ids.extend(inner.held.iter().map(|n| n.id));
        inner.items.clear();
        inner.held.clear();
        drop(inner);
        for &id in &ids {
            let _ = self.close_tx.send(ClosedEvent { id, reason });
        }
        if !ids.is_empty() {
            let _ = self.change_tx.send(());
        }
        ids
    }

    /// Snapshot the current active set as minimal wire structs.
    pub async fn snapshot(&self) -> Vec<MinimalNotification> {
        let inner = self.inner.read().await;
        inner.items.iter().map(|n| n.to_minimal()).collect()
    }

    /// Number of active notifications.
    pub async fn len(&self) -> usize {
        self.inner.read().await.items.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.items.is_empty()
    }
}

fn alloc_id(next_id: &mut u32) -> u32 {
    let id = *next_id;
    *next_id = next_id.wrapping_add(1);
    if *next_id == 0 {
        *next_id = 1;
    }
    id
}

fn find_by_id(items: &[Notification], id: u32) -> Option<&Notification> {
    items.iter().find(|n| n.id == id)
}

fn remove_by_id(items: &mut Vec<Notification>, id: u32) -> bool {
    if let Some(pos) = items.iter().position(|n| n.id == id) {
        items.remove(pos);
        true
    } else {
        false
    }
}

fn assign_id(inner: &mut QueueInner, notif: &mut Notification) -> u32 {
    if notif.replaces_id > 0 {
        let exists = inner.items.iter().any(|n| n.id == notif.replaces_id)
            || inner.held.iter().any(|n| n.id == notif.replaces_id);
        if exists {
            notif.id = notif.replaces_id;
            return notif.replaces_id;
        }
    }
    let id = alloc_id(&mut inner.next_id);
    notif.id = id;
    id
}

fn insert_or_replace(items: &mut Vec<Notification>, notif: Notification) {
    let replace_id = if notif.replaces_id > 0 {
        notif.replaces_id
    } else {
        notif.id
    };
    if let Some(pos) = items.iter().position(|n| n.id == replace_id) {
        items[pos] = notif;
    } else {
        items.push(notif);
    }
}

#[cfg(test)]
mod tests;
