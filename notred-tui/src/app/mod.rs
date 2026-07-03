//! Application state and keyboard handling.

use crate::ctl::{Ctl, CtlError, SubscribeEvent};
use crate::model::HistoryRow;

/// Navigation stack for the manager UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    /// History list (primary).
    List,
    /// Pick an action key for the selected notification.
    Actions {
        row_id: u32,
        selected: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOp {
    None,
    Quit,
    Refresh,
    Up,
    Down,
    Back,
    Forward,
    Enter,
    Remove,
}

pub struct App {
    pub rows: Vec<HistoryRow>,
    pub selected: usize,
    pub view: View,
    pub status: String,
    ctl: Ctl,
}

impl App {
    pub fn new(ctl: Ctl) -> Self {
        Self {
            rows: Vec::new(),
            selected: 0,
            view: View::List,
            status: String::new(),
            ctl,
        }
    }

    pub fn refresh(&mut self) {
        match self.ctl.list_history() {
            Ok(rows) => {
                self.rows = rows;
                if self.selected >= self.rows.len() {
                    self.selected = self.rows.len().saturating_sub(1);
                }
                if self.status != "__quit__" {
                    self.status.clear();
                }
            }
            Err(CtlError::Command(msg)) => self.status = msg,
            Err(e) => self.status = e.to_string(),
        }
    }

    pub fn handle_key(&mut self, op: KeyOp) {
        match op {
            KeyOp::None => {}
            KeyOp::Quit => self.status = "__quit__".into(),
            KeyOp::Refresh => self.refresh(),
            KeyOp::Up => self.move_up(),
            KeyOp::Down => self.move_down(),
            KeyOp::Back => self.back(),
            KeyOp::Forward => self.forward(),
            KeyOp::Enter => self.enter(),
            KeyOp::Remove => self.remove_selected(),
        }
    }

    pub fn handle_subscribe(&mut self, ev: SubscribeEvent) {
        match ev {
            SubscribeEvent::Refresh => self.refresh(),
            SubscribeEvent::Disconnected(msg) => {
                self.status = format!("subscribe ended: {msg}");
            }
        }
    }

    fn move_up(&mut self) {
        match &mut self.view {
            View::List => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            View::Actions { selected, .. } => {
                if *selected > 0 {
                    *selected -= 1;
                }
            }
        }
    }

    fn move_down(&mut self) {
        match &mut self.view {
            View::List => {
                if self.selected + 1 < self.rows.len() {
                    self.selected += 1;
                }
            }
            View::Actions { row_id, selected } => {
                let len = keys_for_id(&self.rows, *row_id).len();
                if *selected + 1 < len {
                    *selected += 1;
                }
            }
        }
    }

    fn back(&mut self) {
        if matches!(self.view, View::Actions { .. }) {
            self.view = View::List;
        }
    }

    fn forward(&mut self) {
        let View::List = &self.view else {
            return;
        };
        let Some(row) = self.rows.get(self.selected) else {
            return;
        };
        if row.has_actions && !row.action_keys.is_empty() {
            self.view = View::Actions {
                row_id: row.id,
                selected: 0,
            };
        }
    }

    fn enter(&mut self) {
        match &self.view {
            View::List => {
                let Some(row) = self.rows.get(self.selected).cloned() else {
                    return;
                };
                if row.has_actions && !row.action_keys.is_empty() {
                    self.view = View::Actions {
                        row_id: row.id,
                        selected: 0,
                    };
                } else {
                    self.run_activate(row.id, None);
                }
            }
            View::Actions { row_id, selected } => {
                let keys = keys_for_id(&self.rows, *row_id);
                let key = keys.get(*selected).cloned();
                self.run_activate(*row_id, key.as_deref());
            }
        }
    }

    fn remove_selected(&mut self) {
        let View::List = &self.view else {
            return;
        };
        let Some(id) = self.rows.get(self.selected).map(|r| r.id) else {
            return;
        };
        match self.ctl.remove(id) {
            Ok(()) => {
                self.refresh();
                self.status = format!("removed {id}");
            }
            Err(e) => self.status = e.to_string(),
        }
    }

    fn run_activate(&mut self, id: u32, key: Option<&str>) {
        match self.ctl.activate(id, key) {
            Ok(()) => {
                self.status = format!("activated {id}");
                self.view = View::List;
            }
            Err(e) => self.status = e.to_string(),
        }
    }

    pub fn selected_row(&self) -> Option<&HistoryRow> {
        self.rows.get(self.selected)
    }

    pub fn action_menu(&self) -> Option<(&HistoryRow, usize)> {
        let View::Actions { row_id, selected } = &self.view else {
            return None;
        };
        let row = self.rows.iter().find(|r| r.id == *row_id)?;
        Some((row, *selected))
    }

    pub fn should_quit(&self) -> bool {
        self.status == "__quit__"
    }
}

fn keys_for_id(rows: &[HistoryRow], id: u32) -> Vec<String> {
    rows.iter()
        .find(|r| r.id == id)
        .map(|r| {
            if r.action_keys.is_empty() {
                vec!["default".into()]
            } else {
                r.action_keys.clone()
            }
        })
        .unwrap_or_default()
}

/// Map crossterm key to operation (arrows primary; vim aliases secondary).
pub fn map_key(key: ratatui::crossterm::event::KeyEvent) -> KeyOp {
    use ratatui::crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

    if key.kind != KeyEventKind::Press {
        return KeyOp::None;
    }

    match key.code {
        KeyCode::Char('q') => KeyOp::Quit,
        KeyCode::Char('j') | KeyCode::Down => KeyOp::Down,
        KeyCode::Char('k') | KeyCode::Up => KeyOp::Up,
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => KeyOp::Back,
        KeyCode::Char('l') | KeyCode::Right => KeyOp::Forward,
        KeyCode::Enter => KeyOp::Enter,
        KeyCode::Delete | KeyCode::Char('d') => KeyOp::Remove,
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => KeyOp::Refresh,
        _ => KeyOp::None,
    }
}

#[cfg(test)]
mod tests;
