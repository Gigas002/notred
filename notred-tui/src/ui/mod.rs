//! ratatui layout and widgets.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::{App, View};

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(4),
            Constraint::Length(2),
        ])
        .split(area);

    draw_header(frame, chunks[0], app);
    match &app.view {
        View::List => draw_list(frame, chunks[1], app),
        View::Actions { .. } => draw_actions(frame, chunks[1], app),
    }
    draw_detail(frame, chunks[2], app);
    draw_footer(frame, chunks[3]);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let title = format!(" notred-tui — {} notification(s) ", app.rows.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(Color::Cyan));
    let status = if app.status.is_empty() || app.status == "__quit__" {
        Line::from(" connected via notredctl ")
    } else {
        Line::from(format!(" {} ", app.status))
    };
    frame.render_widget(Paragraph::new(status).block(block), area);
}

fn draw_list(frame: &mut Frame, area: Rect, app: &App) {
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
        Span::styled("APP             ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("STATE   ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("SUMMARY", Style::default().add_modifier(Modifier::BOLD)),
    ]));
    let mut all_items = vec![header];
    all_items.extend(items);

    let list = List::new(all_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" History "),
    );
    frame.render_widget(list, area);
}

fn draw_actions(frame: &mut Frame, area: Rect, app: &App) {
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

fn draw_detail(frame: &mut Frame, area: Rect, app: &App) {
    let text = app
        .selected_row()
        .map(|r| {
            format!(
                "#{id}  {urgency}  {app_id}\n{body}",
                id = r.id,
                urgency = r.urgency_label(),
                app_id = r.app_id,
                body = r.body,
            )
        })
        .unwrap_or_else(|| "No notifications.".into());
    let block = Block::default().borders(Borders::ALL).title(" Detail ");
    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn draw_footer(frame: &mut Frame, area: Rect) {
    let help = " ↑/↓ move  Enter open/activate  Esc/← back  → actions  d remove  q quit ";
    frame.render_widget(
        Paragraph::new(help).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}

#[cfg(test)]
mod tests;
