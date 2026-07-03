use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::app::App;
use crate::ui::util::truncate;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let Some((row, selected)) = app.action_menu() else {
        return;
    };
    let keys = if row.action_keys.is_empty() {
        vec!["default".to_string()]
    } else {
        row.action_keys.clone()
    };
    let items: Vec<ListItem> = keys
        .iter()
        .enumerate()
        .map(|(i, key)| {
            let marker = if i == selected { "▸" } else { " " };
            let style = if i == selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(format!("{marker} {key}")).style(style)
        })
        .collect();
    let title = format!(" Actions — #{} {} ", row.id, truncate(&row.summary, 30));
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(list, area);
}
