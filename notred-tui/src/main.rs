//! Phase 0 stub — full manager lands in Phase 4 (`docs/PLAN.md`).

use std::process::ExitCode;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "notred-tui",
    version,
    about = "notred terminal manager (not yet implemented)"
)]
struct Cli {}

fn main() -> ExitCode {
    let _ = Cli::parse();
    eprintln!("notred-tui is not implemented yet (see docs/PLAN.md Phase 4).");
    eprintln!("Use `notredctl subscribe` and `notredctl list` for now.");
    ExitCode::from(2)
}
