//! History row shapes from `notredctl list-history` JSON stdout.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryRow {
    pub id: u32,
    pub app_id: String,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
    pub timeout_ms: i32,
    /// Not displayed; kept as JSON so any icon shape (name, path, or raw
    /// `image-data` pixels) parses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<serde_json::Value>,
    pub has_actions: bool,
    #[serde(default)]
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

impl HistoryRow {
    pub fn state_label(&self) -> &'static str {
        match self.state {
            HistoryState::Active => "active",
            HistoryState::Closed => "closed",
        }
    }

    pub fn urgency_label(&self) -> &'static str {
        match self.urgency {
            Urgency::Low => "low",
            Urgency::Normal => "normal",
            Urgency::Critical => "critical",
        }
    }
}

#[cfg(test)]
mod tests;
