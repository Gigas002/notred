//! Session notification history (SQLite, `history` feature).

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, params};

use crate::model::Notification;
use crate::wire::{HistoryRow, HistoryState, Urgency};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS notifications (
    id            INTEGER PRIMARY KEY,
    app_id        TEXT NOT NULL,
    summary       TEXT NOT NULL,
    body          TEXT NOT NULL,
    urgency       TEXT NOT NULL,
    timeout_ms    INTEGER NOT NULL,
    icon_json     TEXT,
    action_keys   TEXT NOT NULL DEFAULT '[]',
    has_actions   INTEGER NOT NULL DEFAULT 0,
    received_at   INTEGER NOT NULL,
    state         TEXT NOT NULL DEFAULT 'active'
);
CREATE INDEX IF NOT EXISTS idx_notifications_received_at ON notifications(received_at);
CREATE INDEX IF NOT EXISTS idx_notifications_state ON notifications(state);
";

/// Query filters for [`HistoryStore::list`].
#[derive(Debug, Clone, Default)]
pub struct HistoryFilter {
    pub active_only: bool,
    pub app_id: Option<String>,
    pub since: Option<i64>,
}

/// SQLite-backed notification log.
pub struct HistoryStore {
    conn: Mutex<Connection>,
}

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl HistoryStore {
    /// Open or create the history database at `path`.
    ///
    /// When `flush` is true, all rows are deleted before returning.
    pub fn open(path: &Path, flush: bool) -> Result<Self, HistoryError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        if flush {
            store.wipe()?;
        }
        Ok(store)
    }

    /// Delete every row (startup flush).
    pub fn wipe(&self) -> Result<(), HistoryError> {
        let conn = self.conn.lock().expect("history db mutex poisoned");
        conn.execute("DELETE FROM notifications", [])?;
        Ok(())
    }

    /// Insert or replace a row for a new `Notify`.
    pub fn upsert_active(&self, notif: &Notification) -> Result<(), HistoryError> {
        let conn = self.conn.lock().expect("history db mutex poisoned");
        let icon_json = notif.icon.as_ref().map(serde_json::to_string).transpose()?;
        let action_keys = serde_json::to_string(&notif.action_keys)?;
        let urgency = urgency_str(notif.urgency);

        conn.execute(
            "INSERT INTO notifications (
                id, app_id, summary, body, urgency, timeout_ms,
                icon_json, action_keys, has_actions, received_at, state
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'active')
            ON CONFLICT(id) DO UPDATE SET
                app_id = excluded.app_id,
                summary = excluded.summary,
                body = excluded.body,
                urgency = excluded.urgency,
                timeout_ms = excluded.timeout_ms,
                icon_json = excluded.icon_json,
                action_keys = excluded.action_keys,
                has_actions = excluded.has_actions,
                received_at = excluded.received_at,
                state = 'active'",
            params![
                notif.id,
                notif.app_id,
                notif.summary,
                notif.body,
                urgency,
                notif.timeout_ms,
                icon_json,
                action_keys,
                i32::from(notif.has_actions),
                notif.timestamp,
            ],
        )?;
        Ok(())
    }

    /// Mark a notification as closed (row retained).
    pub fn mark_closed(&self, id: u32) -> Result<(), HistoryError> {
        let conn = self.conn.lock().expect("history db mutex poisoned");
        conn.execute(
            "UPDATE notifications SET state = 'closed' WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    /// Delete a row. Returns whether a row was removed.
    pub fn remove(&self, id: u32) -> Result<bool, HistoryError> {
        let conn = self.conn.lock().expect("history db mutex poisoned");
        let n = conn.execute("DELETE FROM notifications WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    /// List rows matching optional filters, newest `received_at` first.
    pub fn list(&self, filter: &HistoryFilter) -> Result<Vec<HistoryRow>, HistoryError> {
        let conn = self.conn.lock().expect("history db mutex poisoned");
        let mut sql = String::from(
            "SELECT id, app_id, summary, body, urgency, timeout_ms, icon_json, action_keys, has_actions, received_at, state FROM notifications WHERE 1=1",
        );
        let mut bind: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if filter.active_only {
            sql.push_str(" AND state = 'active'");
        }
        if let Some(app) = &filter.app_id {
            sql.push_str(&format!(" AND app_id = ?{}", bind.len() + 1));
            bind.push(Box::new(app.clone()));
        }
        if let Some(since) = filter.since {
            sql.push_str(&format!(" AND received_at >= ?{}", bind.len() + 1));
            bind.push(Box::new(since));
        }
        sql.push_str(" ORDER BY received_at DESC");

        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> = bind.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(params.as_slice(), row_to_history)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(HistoryError::from)
    }

    /// Drop oldest rows until at most `max` remain (`max == 0` = unlimited).
    pub fn enforce_cap(&self, max: u32) -> Result<(), HistoryError> {
        if max == 0 {
            return Ok(());
        }
        let conn = self.conn.lock().expect("history db mutex poisoned");
        conn.execute(
            "DELETE FROM notifications WHERE id IN (
                SELECT id FROM notifications
                ORDER BY received_at ASC
                LIMIT MAX(0, (SELECT COUNT(*) FROM notifications) - ?1)
            )",
            params![max],
        )?;
        Ok(())
    }
}

fn row_to_history(row: &rusqlite::Row<'_>) -> Result<HistoryRow, rusqlite::Error> {
    let icon_json: Option<String> = row.get(6)?;
    let icon = icon_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
        })?;
    let action_keys: String = row.get(7)?;
    let action_keys: Vec<String> = serde_json::from_str(&action_keys).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let state_str: String = row.get(10)?;
    let state = match state_str.as_str() {
        "active" => HistoryState::Active,
        _ => HistoryState::Closed,
    };
    Ok(HistoryRow {
        id: row.get::<_, i64>(0)? as u32,
        app_id: row.get(1)?,
        summary: row.get(2)?,
        body: row.get(3)?,
        urgency: parse_urgency(row.get::<_, String>(4)?),
        timeout_ms: row.get(5)?,
        icon,
        has_actions: row.get::<_, i32>(8)? != 0,
        action_keys,
        received_at: row.get(9)?,
        state,
    })
}

fn urgency_str(u: Urgency) -> &'static str {
    match u {
        Urgency::Low => "low",
        Urgency::Normal => "normal",
        Urgency::Critical => "critical",
    }
}

fn parse_urgency(s: String) -> Urgency {
    match s.as_str() {
        "low" => Urgency::Low,
        "critical" => Urgency::Critical,
        _ => Urgency::Normal,
    }
}

/// Default history DB path: `$XDG_CACHE_HOME/notred/history.db`.
pub fn default_history_path() -> std::path::PathBuf {
    let base = std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        format!("{home}/.cache")
    });
    std::path::PathBuf::from(base)
        .join("notred")
        .join("history.db")
}

#[cfg(test)]
mod tests;
