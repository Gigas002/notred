//! Spawn `notredctl` on `$PATH` — the only supported integration surface.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use serde::Deserialize;

use crate::model::HistoryRow;

#[derive(Debug, thiserror::Error)]
pub enum CtlError {
    #[error("notredctl not found on $PATH (install notredctl with history enabled)")]
    NotFound,

    #[error("notredctl failed: {0}")]
    Command(String),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Events from the background `notredctl subscribe` reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscribeEvent {
    /// Queue or history changed — refresh `list-history`.
    Refresh,
    /// Subscribe stream ended or errored.
    Disconnected(String),
}

pub struct Ctl {
    program: PathBuf,
    socket: Option<PathBuf>,
}

impl Ctl {
    pub fn new(program: impl Into<PathBuf>, socket: Option<PathBuf>) -> Self {
        Self {
            program: program.into(),
            socket,
        }
    }

    fn base(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.stdin(Stdio::null());
        if let Some(socket) = &self.socket {
            cmd.arg("--socket").arg(socket);
        }
        cmd
    }

    pub fn ping(&self) -> Result<(), CtlError> {
        let output = self.base().arg("ping").output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CtlError::NotFound
            } else {
                CtlError::Io(e)
            }
        })?;
        if output.status.success() {
            Ok(())
        } else {
            Err(CtlError::Command(stderr_lossy(&output.stderr)))
        }
    }

    pub fn list_history(&self) -> Result<Vec<HistoryRow>, CtlError> {
        let output = self.base().arg("list-history").output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CtlError::NotFound
            } else {
                CtlError::Io(e)
            }
        })?;
        if !output.status.success() {
            let msg = stderr_lossy(&output.stderr);
            if msg.contains("history") {
                return Err(CtlError::Command(
                    "history unavailable — build notred/notredctl with the history feature and set [history] enabled = true".into(),
                ));
            }
            return Err(CtlError::Command(msg));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(serde_json::from_str(stdout.trim())?)
    }

    pub fn remove(&self, id: u32) -> Result<(), CtlError> {
        let output = self
            .base()
            .args(["remove", &id.to_string()])
            .output()
            .map_err(map_io)?;
        if output.status.success() {
            Ok(())
        } else {
            Err(CtlError::Command(stderr_lossy(&output.stderr)))
        }
    }

    pub fn activate(&self, id: u32, key: Option<&str>) -> Result<(), CtlError> {
        let mut cmd = self.base();
        cmd.arg("activate").arg(id.to_string());
        if let Some(key) = key {
            cmd.arg(key);
        }
        let output = cmd.output().map_err(map_io)?;
        if output.status.success() {
            Ok(())
        } else {
            Err(CtlError::Command(stderr_lossy(&output.stderr)))
        }
    }

    /// Spawn `notredctl subscribe` and forward refresh events on `tx`.
    pub fn spawn_subscribe(&self, tx: mpsc::Sender<SubscribeEvent>) -> Result<JoinHandle<()>, CtlError> {
        let mut child = self
            .base()
            .arg("subscribe")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(map_io)?;
        let stdout = child.stdout.take().ok_or_else(|| {
            CtlError::Command("subscribe: no stdout".into())
        })?;
        let handle = thread::spawn(move || {
            read_subscribe(stdout, tx);
            let _ = child.wait();
        });
        Ok(handle)
    }
}

fn read_subscribe<R: std::io::Read>(read: R, tx: mpsc::Sender<SubscribeEvent>) {
    let reader = BufReader::new(read);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                let _ = tx.send(SubscribeEvent::Disconnected(e.to_string()));
                return;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(resp) = serde_json::from_str::<SubscribeLine>(&line)
            && resp.needs_refresh()
        {
            let _ = tx.send(SubscribeEvent::Refresh);
        }
    }
    let _ = tx.send(SubscribeEvent::Disconnected(
        "subscribe stream ended".into(),
    ));
}

#[derive(Debug, Deserialize)]
pub(crate) struct SubscribeLine {
    #[serde(rename = "type")]
    line_type: String,
    event: Option<SubscribeEventBody>,
}

#[derive(Debug, Deserialize)]
struct SubscribeEventBody {
    kind: String,
}

impl SubscribeLine {
    pub(crate) fn needs_refresh(&self) -> bool {
        self.line_type == "event"
            && self
                .event
                .as_ref()
                .is_some_and(|e| e.kind == "update" || e.kind == "history_changed")
    }
}

fn stderr_lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

fn map_io(e: std::io::Error) -> CtlError {
    if e.kind() == std::io::ErrorKind::NotFound {
        CtlError::NotFound
    } else {
        CtlError::Io(e)
    }
}

#[cfg(test)]
mod tests;
