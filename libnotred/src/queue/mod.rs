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
    pub async fn push(&self, mut notif: Notification) -> u32 {
        let mut inner = self.inner.write().await;

        let id = if notif.replaces_id > 0 {
            if let Some(pos) = inner.items.iter().position(|n| n.id == notif.replaces_id) {
                let reused = notif.replaces_id;
                notif.id = reused;
                inner.items[pos] = notif;
                reused
            } else {
                let id = alloc_id(&mut inner.next_id);
                notif.id = id;
                inner.items.push(notif);
                id
            }
        } else {
            let id = alloc_id(&mut inner.next_id);
            notif.id = id;
            inner.items.push(notif);
            id
        };

        drop(inner);
        let _ = self.change_tx.send(());
        id
    }

    /// Close one notification. Returns `false` if the id was not found.
    pub async fn close(&self, id: u32, reason: CloseReason) -> bool {
        let mut inner = self.inner.write().await;
        if let Some(pos) = inner.items.iter().position(|n| n.id == id) {
            inner.items.remove(pos);
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
        let ids: Vec<u32> = inner.items.iter().map(|n| n.id).collect();
        inner.items.clear();
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

#[cfg(test)]
mod tests;
