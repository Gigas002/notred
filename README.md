# notred

Session notification platform for Linux — a Freedesktop Notifications daemon with a stable **`notredctl`** CLI and optional terminal manager.

## Components

| Binary / crate | Role |
| -------------- | ---- |
| **`notred`** | Daemon: FDN server + IPC server |
| **`notredctl`** | **Only supported** external connector (CLI + JSON stdout) |
| **`notred-tui`** | Optional full-screen manager (spawns `notredctl` only) |
| **`libnotred`** | Daemon library ([crates.io](https://crates.io/crates/libnotred) — for in-tree / daemon authors, not UI apps) |

**Integrators:** use **`notredctl`**, not the Unix socket directly. Wire protocol details: [`docs/IPC.md`](docs/IPC.md).

## Install

### From source

```bash
git clone https://github.com/Gigas002/notred.git
cd notred

# Minimal daemon + ctl (no SQLite history):
cargo install --path notred --locked
cargo install --path notredctl --locked

# With session history + TUI:
cargo install --path notred --locked --features history
cargo install --path notredctl --locked --features history
cargo install --path notred-tui --locked
```

Release tarballs for `notred`, `notredctl`, and `notred-tui` are attached to [GitHub Releases](https://github.com/Gigas002/notred/releases) when a `v*` tag is pushed.

### systemd (user session)

```bash
mkdir -p ~/.config/systemd/user
cp examples/notred.service ~/.config/systemd/user/notred.service
# Edit ExecStart= if `notred` is not on your PATH.
systemctl --user daemon-reload
systemctl --user enable --now notred.service
```

See [`examples/notred.service`](examples/notred.service) for the unit file and comments.

## Paths (defaults)

| Resource | Location |
| -------- | -------- |
| IPC socket | `$XDG_RUNTIME_DIR/notred.sock` |
| Daemon config | `$XDG_CONFIG_HOME/notred/notred.toml` |
| TUI config | `$XDG_CONFIG_HOME/notred/tui.toml` |
| History DB | `$XDG_CACHE_HOME/notred/history.db` (with `history` feature + `[history] enabled = true`) |

Example configs: [`examples/config.toml`](examples/config.toml), [`examples/tui.toml`](examples/tui.toml).

Override socket for one invocation: `notredctl --socket /path/to/notred.sock …`.

## `notredctl` (subscriber API)

```text
notredctl ping
notredctl list              # active notifications → JSON
notredctl subscribe         # NDJSON event stream on stdout
notredctl close <id>
notredctl close-all
notredctl reload
notredctl pause | unpause
notredctl activate <id> [key]
notredctl list-history      # requires history feature + [history] enabled
notredctl remove <id>       # requires history feature + [history] enabled
```

Run `notredctl --help` for flags. **Do not** open `notred.sock` from application code — wrap `notredctl` or shell scripts instead.

## History (optional)

History is **off** by default at build time and in config.

```bash
cargo build -p notred -p notredctl --features history
```

In `~/.config/notred/notred.toml`:

```toml
[history]
enabled = true
flush = true      # wipe DB on each daemon start
max_entries = 5   # 0 = unlimited
```

## Documentation

- [IPC v1 (implementors)](docs/IPC.md)
- [Repository architecture](docs/ARCHITECTURE.md)

## License

GPL-3.0-only — see [LICENSE](LICENSE).
