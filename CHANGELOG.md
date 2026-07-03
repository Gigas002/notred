# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

First public release of the notred platform.

### Added

- **`notred`** — session Freedesktop Notifications (FDN) daemon on D-Bus (`org.freedesktop.Notifications`) with Unix-socket NDJSON IPC.
- **`notredctl`** — supported control connector (`ping`, `list`, `subscribe`, `close`, `close-all`, `reload`, `pause`, `unpause`, `activate`).
- **`notred-tui`** — ratatui terminal manager; talks to the daemon **only** via `notredctl`.
- **`libnotred`** — daemon-side library (queue, FDN server, IPC server, wire types).
- Optional **`history`** Cargo feature: SQLite session log, `list_history` / `remove` IPC, `history_changed` subscribe events.
- Config: `$XDG_CONFIG_HOME/notred/notred.toml`; TUI config: `$XDG_CONFIG_HOME/notred/tui.toml`.
- IPC v1 spec: [`docs/IPC.md`](docs/IPC.md) and golden fixtures under `examples/ipc-examples/`.
- Example systemd user unit: [`examples/notred.service`](examples/notred.service).

[0.1.0]: https://github.com/Gigas002/notred/releases/tag/v0.1.0
