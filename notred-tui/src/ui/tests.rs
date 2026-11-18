use super::util::truncate;

#[test]
fn truncate_short_unchanged() {
    assert_eq!(truncate("hi", 10), "hi");
}

#[test]
fn truncate_long_adds_ellipsis() {
    assert_eq!(truncate("hello world", 8), "hello w…");
}

mod render {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::app::{App, KeyOp};
    use crate::ctl::Ctl;
    use crate::model::{HistoryRow, HistoryState, Urgency};

    fn row(id: u32, summary: &str, keys: &[&str]) -> HistoryRow {
        HistoryRow {
            id,
            app_id: "mail".into(),
            summary: summary.into(),
            body: "the body text".into(),
            urgency: Urgency::Critical,
            timeout_ms: 0,
            icon: None,
            has_actions: !keys.is_empty(),
            action_keys: keys.iter().map(|k| (*k).to_owned()).collect(),
            received_at: 0,
            state: HistoryState::Closed,
        }
    }

    fn screen(app: &App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| super::super::draw(f, app)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    + "\n"
            })
            .collect()
    }

    #[test]
    fn the_list_shows_rows_and_the_selected_detail() {
        let mut app = App::new(Ctl::new("notredctl", None));
        app.rows = vec![row(7, "Inbox: 3 new", &[]), row(8, "Second", &[])];
        app.status = "removed 9".into();
        let text = screen(&app);
        for needle in [
            "Inbox: 3 new",
            "Second",
            "mail",
            "the body text",
            "removed 9",
        ] {
            assert!(text.contains(needle), "no {needle:?} in\n{text}");
        }
    }

    #[test]
    fn the_actions_view_lists_the_keys() {
        let mut app = App::new(Ctl::new("notredctl", None));
        app.rows = vec![row(7, "Chat", &["reply", "mute"])];
        app.handle_key(KeyOp::Enter);
        let text = screen(&app);
        for needle in ["reply", "mute", "Chat"] {
            assert!(text.contains(needle), "no {needle:?} in\n{text}");
        }
    }

    #[test]
    fn an_empty_history_renders() {
        let app = App::new(Ctl::new("notredctl", None));
        let text = screen(&app);
        assert!(!text.trim().is_empty());
    }
}
