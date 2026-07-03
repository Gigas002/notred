//! Auto-dismiss timers from FDN `expire_timeout`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::task::AbortHandle;

use crate::model::CloseReason;
use crate::queue::{ClosedEvent, Queue};

/// Compute effective timeout in milliseconds (`None` = no auto-dismiss).
pub fn effective_timeout_ms(timeout_ms: i32, default_timeout_ms: u32) -> Option<u64> {
    match timeout_ms {
        0 => None,
        n if n > 0 => Some(n as u64),
        _ => {
            if default_timeout_ms == 0 {
                None
            } else {
                Some(default_timeout_ms as u64)
            }
        }
    }
}

/// Per-notification expiry timers; cancelled when a notification closes early.
pub struct TimeoutManager {
    queue: Arc<Queue>,
    tasks: RwLock<HashMap<u32, AbortHandle>>,
}

impl TimeoutManager {
    pub fn new(queue: Arc<Queue>) -> Self {
        Self {
            queue,
            tasks: RwLock::new(HashMap::new()),
        }
    }

    /// Subscribe to queue close events and cancel pending timers.
    pub fn spawn_cancel_task(
        self: Arc<Self>,
        mut close_rx: tokio::sync::broadcast::Receiver<ClosedEvent>,
    ) {
        tokio::spawn(async move {
            loop {
                match close_rx.recv().await {
                    Ok(ev) => self.cancel(ev.id).await,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    /// Schedule auto-dismiss for `id`, replacing any existing timer for that id.
    pub async fn schedule(&self, id: u32, timeout_ms: i32, default_timeout_ms: u32) {
        let Some(ms) = effective_timeout_ms(timeout_ms, default_timeout_ms) else {
            self.cancel(id).await;
            return;
        };

        self.cancel(id).await;

        let queue = Arc::clone(&self.queue);
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(ms)).await;
            queue.close(id, CloseReason::Expired).await;
        });

        self.tasks.write().await.insert(id, handle.abort_handle());
    }

    pub async fn cancel(&self, id: u32) {
        if let Some(handle) = self.tasks.write().await.remove(&id) {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests;
