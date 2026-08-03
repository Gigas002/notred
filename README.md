# notred

Session notification platform for Linux — a Freedesktop Notifications daemon with a stable **`notredctl`** CLI and optional terminal manager.

## Components

| Binary / crate   | Role                                                                                                         |
| ---------------- | ------------------------------------------------------------------------------------------------------------ |
| **`notred`**     | Daemon: FDN server + IPC server                                                                              |
| **`notredctl`**  | **Only supported** external connector (CLI + JSON stdout)                                                    |
| **`notred-tui`** | Optional full-screen manager (spawns `notredctl` only)                                                       |
| **`libnotred`**  | Daemon library ([crates.io](https://crates.io/crates/libnotred) — for in-tree / daemon authors, not UI apps) |

**Integrators:** use **`notredctl`**, not the Unix socket directly. Wire protocol details: [`docs/IPC.md`](docs/IPC.md).

## Paths (defaults)

| Resource      | Location                                                                                  |
| ------------- | ----------------------------------------------------------------------------------------- |
| IPC socket    | `$XDG_RUNTIME_DIR/notred.sock`                                                            |
| Daemon config | `$XDG_CONFIG_HOME/notred/notred.toml`                                                     |
| TUI config    | `$XDG_CONFIG_HOME/notred/tui.toml`                                                        |
| History DB    | `$XDG_CACHE_HOME/notred/history.db` (with `history` feature + `[history] enabled = true`) |

Example configs: [`examples/config.toml`](examples/config.toml), [`examples/tui.toml`](examples/tui.toml).

Override socket for one invocation: `notredctl --socket /path/to/notred.sock …`.

## `notredctl`

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
