use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
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
