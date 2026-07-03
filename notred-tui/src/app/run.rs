//! Terminal event loop and startup.

use std::io;
use std::sync::mpsc;
use std::time::Duration;

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::ExecutableCommand;
use ratatui::crossterm::event::{self, Event};
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use crate::app::{App, map_key};
use crate::ctl::Ctl;
use crate::settings::Settings;

pub fn run(settings: &Settings) -> io::Result<()> {
    let ctl = Ctl::new(settings.ctl.clone(), settings.socket.clone());

    ctl.ping()
        .map_err(|e| io::Error::new(io::ErrorKind::NotConnected, e.to_string()))?;

    let (subscribe_tx, subscribe_rx) = mpsc::channel();
    let _subscribe_handle = ctl
        .spawn_subscribe(subscribe_tx)
        .map_err(|e| io::Error::other(e.to_string()))?;

    let mut app = App::new(ctl);
    app.refresh();
    if app.rows.is_empty() && app.status.contains("history unavailable") {
        eprintln!("{}", app.status);
        eprintln!("Requires notred + notredctl built with the `history` feature.");
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "history required",
        ));
    }

    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let result = run_loop(&mut terminal, &mut app, subscribe_rx);

    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    subscribe_rx: mpsc::Receiver<crate::ctl::SubscribeEvent>,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| crate::ui::draw(f, app))?;

        if app.should_quit() {
            break;
        }

        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
        {
            app.handle_key(map_key(key));
        }

        while let Ok(ev) = subscribe_rx.try_recv() {
            app.handle_subscribe(ev);
        }

        if app.should_quit() {
            break;
        }
    }
    Ok(())
}
