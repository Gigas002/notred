//! ratatui layout and widgets.

mod actions_panel;
mod detail;
mod footer;
mod header;
mod list_panel;
mod util;

use ratatui::Frame;

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
    use ratatui::layout::{Constraint, Direction, Layout};

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

    header::draw(frame, chunks[0], app);
    match &app.view {
        crate::app::View::List => list_panel::draw(frame, chunks[1], app),
        crate::app::View::Actions { .. } => actions_panel::draw(frame, chunks[1], app),
    }
    detail::draw(frame, chunks[2], app);
    footer::draw(frame, chunks[3]);
}

#[cfg(test)]
mod tests;
