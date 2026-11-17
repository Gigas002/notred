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

/// A stand-in `notredctl`: logs its arguments, and answers `list-history`
/// with two rows (`subscribe` prints one update and ends).
mod with_fake_ctl {
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    use crate::app::{App, KeyOp, View};
    use crate::ctl::{Ctl, SubscribeEvent};

    const ROWS: &str = r#"[
      {"id": 2, "app_id": "chat", "summary": "two", "body": "", "urgency": "normal",
       "timeout_ms": 0, "has_actions": true, "action_keys": ["reply"], "received_at": 2,
       "state": "active"},
      {"id": 1, "app_id": "mail", "summary": "one", "body": "", "urgency": "low",
       "timeout_ms": 0, "has_actions": false, "action_keys": [], "received_at": 1,
       "state": "closed"}
    ]"#;

    fn fake_ctl(dir: &Path, fail: &str) -> PathBuf {
        let log = dir.join("calls.log");
        let script = dir.join("notredctl");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"$*\" >> '{log}'\ncase \"$*\" in\n  *{fail}*) echo \"error: nope ($*)\" >&2; exit 1 ;;\n  *list-history*) cat <<'JSON'\n{ROWS}\nJSON\n;;\n  *subscribe*) echo '{{\"v\":1,\"type\":\"event\",\"event\":{{\"kind\":\"update\",\"items\":[]}}}}' ;;\nesac\n",
                log = log.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    /// Writing a script while another test forks can leave it open for
    /// writing in that child, and exec then fails with ETXTBSY: these tests
    /// take turns.
    static SPAWN_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn turn() -> std::sync::MutexGuard<'static, ()> {
        SPAWN_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn calls(dir: &Path) -> String {
        std::fs::read_to_string(dir.join("calls.log")).unwrap_or_default()
    }

    #[test]
    fn refresh_activate_and_remove_call_notredctl() {
        let _turn = turn();
        let dir = tempfile::tempdir().unwrap();
        let ctl = Ctl::new(fake_ctl(dir.path(), "NEVER"), Some("/s.sock".into()));
        ctl.ping().unwrap();
        let mut app = App::new(ctl);
        app.refresh();
        assert_eq!(app.rows.len(), 2);

        // Row 2 has actions: Enter opens them, Enter again runs the first.
        app.handle_key(KeyOp::Enter);
        assert!(matches!(app.view, View::Actions { row_id: 2, .. }));
        app.handle_key(KeyOp::Enter);
        assert_eq!(app.status, "activated 2");
        assert_eq!(app.view, View::List);

        app.handle_key(KeyOp::Down);
        app.handle_key(KeyOp::Remove);
        assert_eq!(app.status, "removed 1");

        let log = calls(dir.path());
        assert!(log.contains("--socket /s.sock activate 2 reply"), "{log}");
        assert!(log.contains("--socket /s.sock remove 1"), "{log}");
    }

    #[test]
    fn errors_show_in_the_status_line() {
        let _turn = turn();
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(Ctl::new(fake_ctl(dir.path(), "remove"), None));
        app.refresh();
        app.handle_key(KeyOp::Remove);
        assert!(app.status.contains("nope"), "{}", app.status);
    }

    #[test]
    fn a_missing_history_feature_is_explained() {
        let _turn = turn();
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(Ctl::new(fake_ctl(dir.path(), "list-history"), None));
        app.refresh();
        assert!(app.status.contains("history feature"), "{}", app.status);
    }

    #[test]
    fn a_missing_notredctl_is_reported() {
        let ctl = Ctl::new("/nonexistent/notredctl", None);
        assert!(ctl.ping().unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn subscribe_forwards_a_refresh_then_the_end() {
        let _turn = turn();
        let dir = tempfile::tempdir().unwrap();
        let ctl = Ctl::new(fake_ctl(dir.path(), "NEVER"), None);
        let (tx, rx) = std::sync::mpsc::channel();
        ctl.spawn_subscribe(tx).unwrap().join().unwrap();
        assert_eq!(rx.recv().unwrap(), SubscribeEvent::Refresh);
        assert!(matches!(
            rx.recv().unwrap(),
            SubscribeEvent::Disconnected(_)
        ));
        let mut app = App::new(Ctl::new(fake_ctl(dir.path(), "NEVER"), None));
        app.handle_subscribe(SubscribeEvent::Refresh);
        assert_eq!(app.rows.len(), 2);
        app.handle_subscribe(SubscribeEvent::Disconnected("gone".into()));
        assert!(app.status.contains("gone"));
    }
}
