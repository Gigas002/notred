use crate::app::{App, KeyOp, View, map_key};
use crate::ctl::Ctl;
use crate::model::{HistoryRow, HistoryState, Urgency};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

fn sample_row(id: u32, actions: bool) -> HistoryRow {
    HistoryRow {
        id,
        app_id: "app".into(),
        summary: format!("summary {id}"),
        body: "body".into(),
        urgency: Urgency::Normal,
        timeout_ms: -1,
        icon: None,
        has_actions: actions,
        action_keys: if actions {
            vec!["yes".into(), "no".into()]
        } else {
            vec![]
        },
        received_at: id as i64,
        state: HistoryState::Active,
    }
}

#[test]
fn map_key_arrows_and_aliases() {
    let down = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    assert_eq!(map_key(down), KeyOp::Down);
    let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    assert_eq!(map_key(j), KeyOp::Down);
}

#[test]
fn enter_opens_actions_menu() {
    let ctl = Ctl::new("notredctl", None);
    let mut app = App::new(ctl);
    app.rows = vec![sample_row(1, true)];
    app.handle_key(KeyOp::Enter);
    assert!(matches!(app.view, View::Actions { row_id: 1, .. }));
}

#[test]
fn back_returns_to_list() {
    let ctl = Ctl::new("notredctl", None);
    let mut app = App::new(ctl);
    app.view = View::Actions {
        row_id: 1,
        selected: 0,
    };
    app.handle_key(KeyOp::Back);
    assert_eq!(app.view, View::List);
}

#[test]
fn quit_sets_flag() {
    let ctl = Ctl::new("notredctl", None);
    let mut app = App::new(ctl);
    let q = KeyEvent::new_with_kind(
        KeyCode::Char('q'),
        KeyModifiers::empty(),
        KeyEventKind::Press,
    );
    app.handle_key(map_key(q));
    assert!(app.should_quit());
}
