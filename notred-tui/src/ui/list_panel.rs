use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::app::App;
use crate::ui::util::truncate;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let marker = if i == app.selected { "▸" } else { " " };
            let state = row.state_label();
            let line = format!(
                "{marker} {:>4}  {:<16}  {:<8}  {}",
                row.id,
                truncate(&row.app_id, 16),
                state,
                truncate(&row.summary, 40),
            );
            let style = if i == app.selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        })
        .collect();

    let header = ListItem::new(Line::from(vec![
        Span::styled("     ID  ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            "APP             ",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled("STATE   ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("SUMMARY", Style::default().add_modifier(Modifier::BOLD)),
    ]));
    let mut all_items = vec![header];
    all_items.extend(items);

    let list =
        List::new(all_items).block(Block::default().borders(Borders::ALL).title(" History "));
    frame.render_widget(list, area);
}
