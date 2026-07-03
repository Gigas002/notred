use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
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
