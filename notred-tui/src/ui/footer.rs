use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Paragraph;

pub fn draw(frame: &mut Frame, area: Rect) {
    let help = " ↑/↓ move  Enter open/activate  Esc/← back  → actions  d remove  q quit ";
    frame.render_widget(
        Paragraph::new(help).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}
