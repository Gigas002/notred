//! NDJSON IPC wire types (`docs/IPC.md` v1).

use serde::{Deserialize, Serialize};

pub const V: u8 = 1;

/// Consumer → daemon request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Request {
    pub v: u8,
    #[serde(flatten)]
    pub cmd: Cmd,
}

impl Request {
    pub fn new(cmd: Cmd) -> Self {
        Self { v: V, cmd }
    }
}

/// Phase 0+ commands (`docs/PLAN.md` §4.1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Cmd {
    Ping,
    Subscribe,
    List,
    /// Dismiss one active notification by id (emits FDN `NotificationClosed`).
    Dismiss {
        id: u32,
    },
    /// Dismiss all active notifications.
    CloseAll,
    /// User chose an action; emits FDN `ActionInvoked` (key defaults to `"default"`).
    Activate {
        id: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
    },
    /// Re-read daemon config from disk.
    Reload,
    /// Stop surfacing new notifications until `unpause`.
    Pause,
    /// Resume surfacing; flush held notifications to subscribers.
    Unpause,
    /// Session history snapshot (`history` feature).
    ListHistory {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        active_only: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        app_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        since: Option<i64>,
    },
    /// Delete a history row; dismiss on FDN if still active (`history` feature).
    Remove {
        id: u32,
    },
}

/// Daemon → consumer response. Tries `Err` variant first on deserialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Response {
    Err(ErrResponse),
    Ok(OkResponse),
}

impl Response {
    pub fn ok(payload: OkPayload) -> Self {
        Self::Ok(OkResponse { v: V, payload })
    }

    pub fn err(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Err(ErrResponse {
            v: V,
            error: ErrorBody {
                code,
                message: message.into(),
            },
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OkResponse {
    pub v: u8,
    #[serde(flatten)]
    pub payload: OkPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrResponse {
    pub v: u8,
    pub error: ErrorBody,
}

/// Successful response payloads, tagged by `"type"`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OkPayload {
    Pong,
    Ok,
    Items { items: Vec<MinimalNotification> },
    History { rows: Vec<HistoryRow> },
    Event { event: Event },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    NotFound,
    NotImplemented,
    InvalidRequest,
}

/// Minimal active-notification snapshot for `update` / `list`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MinimalNotification {
    pub id: u32,
    pub app_id: String,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
    pub timeout_ms: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<IconRef>,
    pub has_actions: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
}

/// History log row for `list_history`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryRow {
    pub id: u32,
    pub app_id: String,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
    pub timeout_ms: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<IconRef>,
    pub has_actions: bool,
    pub action_keys: Vec<String>,
    pub received_at: i64,
    pub state: HistoryState,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HistoryState {
    Active,
    Closed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum IconRef {
    Name { name: String },
    Path { path: String },
}

/// Events pushed on `subscribe`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Update { items: Vec<MinimalNotification> },
    /// Daemon config changed; subscribers should refresh cached policy.
    Reload,
    /// History database mutated (insert, remove, close).
    HistoryChanged,
}

#[cfg(test)]
mod tests;
