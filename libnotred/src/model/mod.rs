//! Server-side notification model types.

use crate::wire::{IconRef, MinimalNotification, Urgency};

/// A notification held in the active queue.
#[derive(Debug, Clone)]
pub struct Notification {
    pub id: u32,
    /// From FDN `Notify`: use to replace an existing notification (0 = new).
    pub replaces_id: u32,
    pub app_id: String,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
    pub timeout_ms: i32,
    pub icon: Option<IconRef>,
    pub action_keys: Vec<String>,
    pub has_actions: bool,
    /// Unix timestamp (seconds) when the notification arrived.
    pub timestamp: i64,
}

impl Notification {
    pub fn to_minimal(&self) -> MinimalNotification {
        MinimalNotification {
            id: self.id,
            app_id: self.app_id.clone(),
            summary: self.summary.clone(),
            body: self.body.clone(),
            urgency: self.urgency,
            timeout_ms: self.timeout_ms,
            icon: self.icon.clone(),
            has_actions: self.has_actions,
            timestamp: Some(self.timestamp),
        }
    }
}

/// FDN `NotificationClosed` reason codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CloseReason {
    Expired = 1,
    DismissedByUser = 2,
    ClosedByCall = 3,
    Undefined = 4,
}

impl From<CloseReason> for u32 {
    fn from(r: CloseReason) -> Self {
        r as u32
    }
}

#[cfg(test)]
mod tests;
