# notred — platform architecture + implementation plan

This document is the **human roadmap** and **agent playbook** for the **notred** notification **platform**: a session host (Freedesktop Notifications on D-Bus + queue + private IPC) with **no in-tree UI**. Popup drawers, bars, and terminal managers are **external clients** — they integrate via **`notredctl`** only.

**Discipline:** library-first crate split, small verifiable phases, strict **quality gates** (§7 — fmt, clippy `-D warnings` with a feature matrix, tests, `cargo doc`, `typos`, `cargo deny`). **Directory modules** with **sibling `tests.rs`** — tests never live in the same file as logic.

**Pronunciation / branding:** **notred** (*NOT-red*) — notification daemon family; **not** [libnotify](https://gitlab.gnome.org/GNOME/libnotify) / GNOME `notify-send` the library.

**Workspace crates:**

| Layer | Crate / binary |
| ----- | -------------- |
| Library | **`libnotred`** |
| Daemon | **`notred`** |
| CLI | **`notredctl`** |
| TUI | **`notred-tui`** |
| External UI | any process that runs **`notredctl`** (not the socket directly) |

---

## 1. Why this architecture

### 1.1 Problem with the monolith model

Most notification daemons (mako, dunst, …) combine in **one process**:

- Own `org.freedesktop.Notifications` on the session bus
- Queue, timeouts, FDN signals
- Wayland layer-shell surfaces + drawing

That is **historical convenience**, not a Freedesktop requirement. The spec defines **one bus name** for apps (`Notify`, `CloseNotification`, signals) — it does **not** require pixels to live in the same binary.

### 1.2 What we want instead

```text
Apps (notify-send, browsers, …)
        │
        │  org.freedesktop.Notifications (session D-Bus)
        ▼
   ┌─────────┐     Unix socket (NDJSON)      ┌──────────────┐
   │ notred  │ ◄────────────────────────────►│  notredctl   │  ← sole connector
   │ daemon  │         (private)             └──────┬───────┘
   └─────────┘                                      │ exec / scripts
                                                    ▼
                                            ┌───────────────────┐
                                            │  UIs, TUI, shell  │
                                            └───────────────────┘
```

- **notred** = FDN, queue, optional history DB, IPC **server**. **Never** draws notification UI.
- **notredctl** = **only supported external API**: implements the socket protocol; exposes CLI (`remove`, `list-history`, `subscribe`, …).
- **notred-tui** = in-repo manager: runs **`notredctl` only** — never opens `history.db` or `notred.sock`.
- **Third-party clients** = shell/Rust/etc. that **`exec notredctl`**. **No `libnotred`**. Direct socket access is **allowed but discouraged** — see §4.6.
- **`docs/IPC.md`** documents the wire format for **`notredctl` / `libnotred` maintainers** and golden tests, not as the primary integration guide for UI authors.

### 1.3 When the split is worth it

| Goal | Split justified? |
| ---- | ---------------- |
| Only mako-like corner popups in one binary | Marginal — extra process + protocol |
| **notred-tui** + multiple subscribers + SSH-friendly management | **Yes** |
| Platform discipline (daemon + ctl + documented IPC) | **Yes** |
| Philosophical decoupling only, one client forever | **No** — ship monolith instead |

This plan assumes **notred-tui** and **at least one external graphical subscriber** are realistic goals for the ecosystem, without specifying which.

---

## 2. Repository and packaging

### 2.1 One workspace

| Repository | Workspace members | Ships |
| ---------- | ----------------- | ----- |
| **`notred`** | `libnotred`, `notred`, `notredctl`, `notred-tui` | Platform: daemon + ctl + TUI + published library |

Do **not** split `notred` / `notredctl` / `notred-tui` / `libnotred` into separate GitHub repos unless release ownership truly differs. Keep one workspace.

### 2.2 Install sets

| User profile | Packages |
| ------------ | -------- |
| Headless / FDN only | `notred` (build without `history` feature for minimal binary) |
| Operator | + `notredctl` |
| Terminal manager | + `notred-tui` (requires `notred` + `notredctl` built **with** `history`) |
| Integrator | `notred` + `notredctl` (+ optional `examples/scripts/` that wrap ctl) |

Graphical popups are **out of scope** for this repo — packaged separately by whoever ships them.

### 2.3 Naming and collisions

| Item | Value |
| ---- | ----- |
| Daemon binary | `notred` |
| Socket (default) | `$XDG_RUNTIME_DIR/notred.sock` |
| Config dir | `$XDG_CONFIG_HOME/notred/` |
| History DB | `$XDG_CACHE_HOME/notred/history.db` (`~/.cache/notred/history.db`) |
| README disclaimer | Unrelated to **libnotify** / GNOME `notify-send` stack |

Check `which notred` and crates.io names before first release; document pronunciation once in README.

---

## 3. Crate boundaries

**`libnotred`** holds FDN + queue + IPC server; the **`notred`** binary is thin wiring (clap, config, tracing).

| Component | Responsibility |
| --------- | -------------- |
| **`libnotred`** | FDN server, queue, IPC server, wire types (`zbus` in library) |
| **`notred`** | Load config, run `libnotred::NotredHost` |
| **`notredctl`** | Sole supported external connector |
| Subscribers | **`notredctl` only** (preferred); scripts wrap ctl, not the socket |

### 3.1 `libnotred`

**Purpose:** **Daemon-side** library only — FDN host, queue, optional history DB, IPC **server**, wire types for tests. **No** Cairo, **no** Wayland, **no** `clap` / **no** `toml` in the library (config parsing stays in the `notred` crate). **Subscribers do not link `libnotred`** and **do not talk to `notred.sock` directly** in the supported design — they run **`notredctl`** (see §4.6).

| Module (target) | Responsibility |
| --------------- | -------------- |
| `wire/` | Request/response/event structs, `serde` JSON lines, `"v":1` (golden tests; IPC.md is the public contract) |
| `model/` | `Notification`, `Urgency`, action keys, close reasons — server-side |
| `dbus/notifications/` | FDN server: `Notify`, `CloseNotification`, `GetCapabilities`, `GetServerInformation`, signals |
| `queue/` | ids, stack metadata, `replaces_id`, pause, `max_visible` policy |
| `timeouts/` | Tokio timers; dismiss → `NotificationClosed` |
| `spawn/` | `tokio::process` for `[events]` hooks (invoked from host, not from subscribers) |
| `ipc/server/` | Unix listener, NDJSON, fan-out `subscribe`, RPC dispatch |
| `host/` (or `daemon/`) | `NotredHost::run` — owns bus name, queue, IPC, bridges D-Bus ↔ subscribers |
| `history/` | Session notification **log** (SQLite) — **`history` feature only**; see §5.1 |

**Cargo features:**

| Feature | Enables | Used by |
| ------- | ------- | ------- |
| `server` (default for `notred` bin) | `dbus/`, `queue/`, `timeouts/`, `spawn/`, `ipc/server/`, `host/`, `wire/` | **`notred`** binary |
| `history` | `history/`, `rusqlite`; IPC `list_history`, `remove`, `history_changed`; ctl subcmds | Full install, **notred-tui** |

Default **`notred`** binary: `features = ["server", "history"]`. Minimal: `cargo build -p notred --no-default-features --features server` — no SQLite, no history RPCs (§5.1).

**`notredctl`** is the **only IPC client** meant for external use: it connects to `notred.sock` and maps every subscriber-facing action to subcommands. Implementation may use `libnotred` (`wire/` + small client module) **inside this workspace** — that code is **not** a public integration API.

Breaking IPC bumps are documented in `docs/IPC.md` + golden fixtures (for daemon + ctl authors); **subscriber-facing stability** is **`notredctl` CLI + JSON stdout**, semver on **`notred`** / **`notredctl`** releases.

### 3.2 `notred` binary

**Purpose:** Thin entrypoint — not a second copy of the host logic.

| Module (target) | Responsibility |
| --------------- | -------------- |
| `main.rs` | clap, tracing-subscriber, exit codes |
| `config/` | Load/merge TOML → plain structs passed into `libnotred::Host` |
| `app.rs` | `HostConfig` + `libnotred::NotredHost::run().await` |

**No** domain logic in the binary beyond config validation and paths (XDG, `--config`, socket override).

**Threading:** Implemented inside **`libnotred`**: D-Bus + timers + IPC on Tokio; push `update` events to subscribers. All UI stays out-of-process.

**Single instance:** `libnotred` requests `org.freedesktop.Notifications`; second `notred` process fails fast if the name is taken.

### 3.3 `notredctl`

**Purpose:** **The connector** between the outside world and **notred** — implements all socket RPCs; exposes a stable CLI. No D-Bus, no Wayland. External processes **must not** duplicate this connector; they invoke **`notredctl`**.

| Command (v0 minimum) | Effect |
| -------------------- | ------ |
| `notredctl ping` | Health check |
| `notredctl reload` | Re-read `notred` config from disk |
| `notredctl close-all` | Dismiss all; emit FDN closed per id |
| `notredctl close <id>` | Dismiss one (active queue) |
| `notredctl pause` / `unpause` | Stop/show new `Notify` (policy TBD) |
| `notredctl list` | Active notifications → JSON on stdout |
| `notredctl list-history` | History rows → JSON (`history` feature + `[history] enabled`) |
| `notredctl remove <id>` | Remove from history; FDN close if still active (`history` feature) |
| `notredctl subscribe` | Block; stream NDJSON **events** (one line per event) on stdout until killed — for TUI/graphical loops |
| `notredctl activate <id> [key]` | `ActionInvoked` path (post–Phase 2) |

Post-v0: modes (mako-style DND) if needed.

**Subscriber contract (preferred):** every action is **`notredctl <subcommand>`** (or a shell script that only calls `notredctl`). **Do not** open `$XDG_RUNTIME_DIR/notred.sock` from UI code. **`libnotred` is not a dependency** of external crates.

### 3.4 `notred-tui`

**Purpose:** ratatui full-screen **notification manager** — ships in this repo as the reference manager UI.

**Role:** **Does not open `history.db` or `notred.sock`.** Runs **`notredctl subscribe`** (pipe/parse JSON lines) for live updates; **`notredctl list-history`**, **`notredctl remove <id>`**, etc. for queries and actions.

**Differentiators** (not possible as a thin layer on monolithic daemons without their in-process UI):

- Browse **all notifications recorded this session** (while notred runs), not only what is on screen right now
- Keyboard: dismiss, activate, open submenu of actions
- Filter by app / urgency; inspect merged daemon config (read-only or trigger `reload`)
- Works without compositor / over SSH
- Pause/unpause, close-all, without a graphical popup client running

**Input policy:** **Arrow keys** (↑/↓/←/→) are **primary** for navigation and drilling into menus. **Vim-style** binds (`j`/`k`, etc.) are **optional aliases only** — never the only way to move; document them in README as convenience, not defaults in help text.

**Keys (v0 sketch):**

| Key | Action |
| ----- | ------ |
| ↑ / ↓ | Move selection |
| ← / → | Back / forward in menu stack (or mirror Esc / Enter where clearer) |
| Enter | Open submenu / activate item |
| Esc | Back one level |
| Delete or `d` | `notredctl remove <id>` |
| `q` | Quit |

Optional aliases (second class): `j`/`k` → same as ↓/↑; `h`/`l` → same as ←/→ if those are bound at all.

Depends: `ratatui`, `crossterm`, **`notredctl` on `$PATH`**; **no** `libnotred`, **no** direct socket, **no** zbus.

---

## 4. IPC protocol (v1)

**Audience:** `docs/IPC.md` + golden fixtures `examples/ipc-examples/` describe the **notred ↔ notredctl** wire protocol (implementors + tests). **UI integrators** read **`notredctl --help`** and JSON stdout shapes — not IPC.md first.

**Transport (internal):** Unix stream `$XDG_RUNTIME_DIR/notred.sock`, **one JSON object per line** (NDJSON), UTF-8, trailing `\n`. Only **`notred`** (server) and **`notredctl`** (client) use this socket in the supported stack.

**Envelope:** every line includes `"v":1`.

### 4.1 Client → server (requests)

| `cmd` | Purpose |
| ----- | ------- |
| `subscribe` | Join fan-out; server streams events until disconnect |
| `ping` | Health |
| `list` | Snapshot of active notifications |
| `get` | Full record for one `id` (body, actions, hints needed to render) |
| `get_icon` | Icon bytes or path handle for one notification (if not inlined in `get`) |
| `dismiss` | User dismissed; notred emits `NotificationClosed` with reason |
| `activate` | User chose action key; notred emits `ActionInvoked` (+ optional `[events]` shell) |
| `close_all` | Operator dismiss all |
| `list_history` | Rows from session DB (filters in IPC.md) |
| `remove` | Delete row from history; if still active, dismiss + FDN `NotificationClosed` |
| `reload` | Re-read notred config |
| `pause` / `unpause` | Control ingestion/display policy |

Extend only with version bump or optional fields ignored by old clients.

### 4.2 Server → client (events)

| `type` | Purpose |
| ------ | ------- |
| `event` | Substream payload |
| `response` | Reply to a request |

**Event kinds (v0):**

| `event.kind` | Payload |
| ------------ | ------- |
| `update` | `items: MinimalNotification[]` — full active set snapshot (simple for v0) |
| `removed` | `id` — optional optimization if moving to delta later |
| `reload` | “config changed”; subscribers refresh cached policy |
| `history_changed` | History DB mutated (insert, remove, flush) — manager clients refresh |

**`MinimalNotification` (v0 fields):**

- `id: u32`
- `app_id: string`
- `summary: string`
- `body: string` (or omit in minimal update + `get` on demand — **choose one in IPC.md**; default recommendation: full body in `update` for simple subscribers until profiling says otherwise)
- `urgency: low | normal | critical`
- `timeout_ms: i32` (`-1` persist)
- `icon: null | { name } | { path }` — resolved by notred when possible
- `has_actions: bool`
- `timestamp` (optional, for TUI sorting)

### 4.3 User activation flow (subscribers)

```text
External UI / script
    │  notredctl remove <id>  |  notredctl activate …  |  notredctl subscribe
    ▼
notredctl  (socket client — only supported external connector)
    ▼
notred  (only process that touches history.db)
    │  optional [events] shell (Tokio)
    │  ActionInvoked / NotificationClosed on D-Bus
    ▼
Apps
```

Ordering for clients like wayshot: document in IPC.md (shell before signal).

### 4.4 Security

- Socket under `$XDG_RUNTIME_DIR` only; mode `0600` on create.
- No authentication in v0 (session user only).
- Document threat model: local user can dismiss/spoof manager; not for multi-user machines.

### 4.5 Why not D-Bus for external UIs?

- FDN is the **app-facing** API. External tools use **`notredctl`**, not a second D-Bus surface and **not** raw socket by default.
- The socket exists so **one** connector (`notredctl`) can be tested and versioned; protocol details stay in **IPC.md** for ctl/daemon authors.

### 4.6 Subscriber integration (`notredctl` only — preferred)

| Need | **Preferred** | Discouraged |
| ---- | ------------- | ----------- |
| Remove id 42 | `notredctl remove 42` | `socat` + manual `remove` JSON to socket |
| List history | `notredctl list-history` | IPC `list_history` from your own client |
| Live updates | `notredctl subscribe` (stdout, one event per line) | Direct `subscribe` on socket |
| Wrapper script | `examples/scripts/*.sh` that **only** invokes `notredctl` | Scripts that open `notred.sock` |
| Custom bar / TUI | `Command::new("notredctl")` / shell | `libnotred`, custom NDJSON client |

**Direct socket access** remains documented in **IPC.md** for debugging and for **notredctl** implementation — not for normal UI integration. **`libnotred`** is daemon-side (+ ctl-internal); external repos **must not** depend on it.

**notred-tui** and all graphical subscribers follow the **Preferred** column only.

---

## 5. Configuration (notred only)

All configuration for **notred** lives in the **notred** repository under `$XDG_CONFIG_HOME/notred/` (see `examples/config.toml` there when the repo exists). Schemas, merge rules, and examples are defined **only** in that project.

**Subscriber configs are external.** Graphical or terminal UIs maintain their own config in their own repos; **notred** does not read them. Integration is **`notredctl`** only.

| Concern (notred only) | Location (notred repo) |
| --------------------- | ---------------------- |
| Queue, `max_visible`, timeouts, pause | `examples/config.toml` |
| Override fragments, `[events]` shell | config + `paths.overrides` |
| `[history]` | same |
| Which notifications are active / visible to subscribers | notred queue policy |
| Icon hint resolution (for IPC) | notred |

**notred** does not read theme files or any subscriber config. Subscribers do not read `$XDG_CONFIG_HOME/notred/`.

**Rule:** If it affects **what apps experience** on D-Bus (timeout, dismiss, signals, capabilities), it belongs in **notred** config. If it affects **pixels only**, it belongs in the **subscriber** repo — never the other way around.

### 5.1 Notification history (optional)

History is an **optional platform feature** — compile-time **`history`** Cargo feature **and** runtime **`[history]`** config. FDN and the active queue work without it.

| Layer | How to disable |
| ----- | -------------- |
| **Compile** | Build without feature: `notred` / `libnotred` with `features = ["server"]` only — no `rusqlite`, no `history/` module, `notredctl list-history` / `remove` exit with clear error |
| **Runtime** | `[history] enabled = false` — daemon runs, no DB writes, history IPC no-ops / errors |

**FDN does not define history.** When enabled, **notred** records at `Notify` time so **notred-tui** can browse notifications that arrived while the TUI was closed. Subscribers are **views/controllers** via **`notredctl`**, not owners of the log.

#### Storage (requires `history` feature)

| Choice | Decision |
| ------ | -------- |
| Engine | **SQLite** (embedded, single-user), via **`rusqlite`** (bundled) — one file, SQL for list/filter/delete |
| Turso / libsql | **Deferred** — not needed for local history; revisit only if cross-machine sync is a goal |
| Path | `$XDG_CACHE_HOME/notred/history.db` (default `~/.cache/notred/history.db`; create parent dir on open) |
| Schema | `libnotred/src/history/` — migrations in repo; columns: id, app_id, summary, body, urgency, timestamps, state (`active` \| `closed`), icon refs, action keys JSON, etc. |

SQLite is used for **structured management** (retention, queries, indexes), **not** for long-term archival across reboots (see flush below).

#### Configuration `[history]`

In `examples/config.toml` (schema source of truth):

```toml
[history]
# Requires notred built with Cargo feature "history". Ignored if feature off.
enabled = true

# Wipe history.db when the notred process starts (before accepting Notify).
#   true  = flush on each daemon start (default)
#   false = keep existing DB contents across restarts of notred
flush = true

# How many notifications to retain (only when enabled = true).
#    0 = no limit (grow until explicit remove or optional startup flush)
#   N>0 = keep at most N rows; on new Notify, delete oldest rows until count <= N
max_entries = 5
```

| Key | Behavior |
| --- | -------- |
| **`enabled`** | **`true`** (default when `history` feature compiled): SQLite + history IPC. **`false`**: no DB writes; `notredctl list-history` / `remove` error or no-op. Ignored if binary lacks `history` feature. |
| **`flush`** | When **`enabled = true`**: **`true`** (default) wipes `history.db` on each **notred** start; **`false`** keeps file. **No timer** — startup only. |
| **`max_entries`** | When **`enabled = true`**: **`0`** = unlimited until `remove` or flush on restart; **`N > 0`** (default **5**) = ring buffer (drop oldest after insert). |

Reloading config re-reads `enabled` / `max_entries`; truncate to new cap on reload (recommended). **`flush` only at process start** — not on `reload`.

#### Lifecycle

| Event | notred behavior |
| ----- | --------------- |
| **Daemon startup** | If history off (no feature or `enabled = false`): skip DB. Else if **`flush = true`**: wipe `history.db`; if **`flush = false`**: open existing file. |
| **`Notify`** | If history **enabled**: insert row, enforce cap, emit `history_changed`. Always emit `update` for active set. |
| **Timeout / `CloseNotification` from app** | Mark row `closed` in DB; **row stays** until explicit `remove` or retention eviction |
| **`notredctl remove`** | Delete row from DB; if still active, close on FDN + `NotificationClosed` (only path for external UIs — never SQLite) |
| **Daemon exit** | DB file remains under cache dir; next start flushes only when **`flush = true`** |

**Retention eviction** (when `max_entries = N > 0`) drops the **oldest** rows by `received_at`. **Explicit remove** is the only way clients shrink history without a new `Notify` pushing out the oldest.

#### IPC surface (Phase 3, `history` feature)

When feature or `enabled = false`: `list_history` / `remove` return structured errors; no `history_changed` events.

- `list_history` — query with optional filters (active only, app_id, since timestamp)
- `remove { id }` — delete from DB + FDN cleanup if needed
- `subscribe` includes `history_changed` when history is active

**Active queue** (`list` / `update` for popup subscribers) remains the “currently surfaced” subset; history is a **superset** for the manager. Document overlap: an active notification is also a history row until removed.

#### notred-tui role (after Phase 3)

Real-time **control panel** for the history DB: browse, filter, activate, dismiss/remove — not a second log. Works when no graphical subscriber runs because **notred** already recorded `Notify` events.

#### Non-goals

- **In-daemon history browser UI** (dunst-style ncurses inside notred) — never; UI is **notred-tui** or third-party clients.
- **Cross-notred-run persistence** — only when **`flush = false`**; default **`flush = true`** clears the cache DB on each notred process start.
- **FDN API for history** — none; notred-specific IPC only.

---

## 6. Freedesktop Notifications

| Item | Value |
| ---- | ----- |
| Bus name | `org.freedesktop.Notifications` |
| Path | `/org/freedesktop/Notifications` |
| Implementation | `libnotred` / `notred` only (`zbus`) |

**v0 methods:** `Notify`, `CloseNotification`, `GetCapabilities`, `GetServerInformation`.

**v0 signals:** `NotificationClosed`, `ActionInvoked`.

**Capabilities:** advertise only what notred implements together with expected subscriber behavior (e.g. `body`, `actions`, `icon-static` when icon pipeline works).

**Actions UI:** notred does not draw action buttons. Subscribers choose presentation; activation is always via IPC `activate` with an action key.

**Non-goals (v0):** inhibition, inline `image-data` body, daemon sound — unless explicitly added later. (Optional history §5.1; **notred-tui** requires history enabled.)

---

## 7. Quality gates

Aligned with [poshanka §7](https://github.com/Gigas002/poshanka/blob/master/docs/PLAN.md#7-quality-gates). Whenever a phase is marked complete, **all** of the following must pass locally and in CI:

- `cargo fmt --all -- --check`
- `typos` (`.github/workflows/typos.yml`)
- `cargo deny check licenses` (`deny.toml` allow list kept current — add entries before enabling new deps, especially `rusqlite` in Phase 3)
- `cargo clippy --workspace --all-targets -- -D warnings` (workspace **default** features)
- `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- After **`history`** exists: `cargo clippy --workspace --all-targets --no-default-features --features server -- -D warnings` (daemon path without SQLite)
- `cargo test --workspace` (default features)
- `cargo test --workspace --no-default-features`
- `cargo test --workspace --all-features`
- `cargo doc --workspace --no-deps --all-features` (`RUSTDOCFLAGS=-D warnings` in CI)

### 7.1 Test discipline

- **Layout:** each directory module has `mod.rs` + sibling **`tests.rs`** — no `#[cfg(test)]` blocks inside `mod.rs` for unit tests.
- **`libnotred`:** queue, wire serde, timeout math, history cap/flush (Phase 3+) — **no compositor**, no Wayland.
- **Golden IPC:** `examples/ipc-examples/*.jsonl` must match `docs/IPC.md`; round-trip tests in `libnotred` / `notred` IPC modules.
- **D-Bus:** prefer `zbus` test bus or documented `#[ignore]` live tests (`cargo test -- --ignored`); document headless strategy in Phase 1 before marking FDN done.
- **Integration:** fake NDJSON client under `examples/fake-client/` (or in-crate test helpers) for subscribe / `list` / `dismiss` without a graphical subscriber.
- **`notredctl` / `notred-tui`:** CLI parse tests, JSON stdout shape tests; TUI tests use mocked `notredctl` output where possible (no ratatui snapshot churn in CI).

### 7.2 CI

Workflows under `.github/workflows/` must cover the full workspace:

| Workflow | Gate |
| -------- | ---- |
| `fmt-clippy.yml` | `cargo fmt --check`; clippy matrix (`--all-features`, `--no-default-features`; extend with `--features server` when `history` lands) |
| `build.yml` | `cargo build --workspace` — default, `--all-features`, `--no-default-features` |
| `test.yml` | `cargo test --workspace` + optional llvm-cov per crate |
| `deny.yml` | `cargo deny check licenses` |
| `typos.yml` | `typos` |
| `doc.yml` | `cargo doc --workspace --no-deps --all-features` |

**Crates in every job:** `libnotred`, `notred`, `notredctl`, `notred-tui`.

**Not required in this repo:** libcairo / libpango / Wayland dev packages (no in-tree rendering). Session D-Bus on Ubuntu runners is sufficient for zbus unit tests; document skips if the runner lacks a bus.

### 7.3 Workspace dependencies (enforced by review + gates)

| Crate | Tokio | zbus | Cairo/Pango | Wayland |
| ----- | ----- | ---- | ----------- | ------- |
| `libnotred` (Phase 0: `ipc` only; Phase 1+: `server`) | yes | Phase 1+ | **no** | **no** |
| `notred` bin | via lib | Phase 1+ | **no** | **no** |
| `notredctl` | minimal (socket I/O) | **no** | **no** | **no** |
| `notred-tui` | **no** (spawn `notredctl` only, Phase 4+) | **no** | **no** | **no** |

**History (Phase 3):** `rusqlite` with `bundled` behind Cargo feature **`history`** only — not linked when `history` is off; `cargo deny` updated in the same PR.

---

## 8. Phased implementation

### Phase 0 — Workspace setup

- [x] Workspace `libnotred`, `notred`, `notredctl`, `notred-tui` (stubs OK).
- [x] **`docs/IPC.md`** + `examples/ipc-examples/*.jsonl` golden lines.
- [x] `libnotred`: wire types (server tests) + IPC server handles `subscribe` / `ping`.
- [x] `notred`: bind `$XDG_RUNTIME_DIR/notred.sock`, handle `subscribe` + `ping`, echo `update` with empty `items`.
- [x] `notredctl`: socket connector; `ping`, `subscribe` (stream events to stdout).
- [x] Optional `examples/scripts/notred-watch.sh` — wraps `notredctl subscribe | jq` (no direct socat to socket).
- [x] CI workflows match §7.2; all §7 gates green on workspace.

**Verify:** `notredctl ping` while `notred` runs; golden IPC round-trip tests; **all gates in §7**.

### Phase 1 — FDN server + queue (`libnotred`)

- [x] `libnotred` (`server`): own bus name, `Notify`, ids, `CloseNotification`, signals.
- [x] Queue + `replaces_id`; on change → IPC `update` event.
- [x] `notred` bin: config load → `HostConfig`.
- [x] `notredctl list`, `close`, `close-all`.

**Verify:** `notify-send` updates `notredctl list`; no subscriber required.

### Phase 2 — IPC RPC: dismiss, activate, reload, pause

- [x] Wire `dismiss` / `activate` → FDN signals + `[events]` shells.
- [x] `reload`, `pause` / `unpause`.
- [x] Document action-key selection (`default` preferred).

**Verify:** `dbus-monitor` on `ActionInvoked`; wayshot-style clients.

### Phase 3 — History store (optional `history` feature)

- [ ] Cargo feature **`history`** on `libnotred` / `notred` / `notredctl` (default **on** for release binaries).
- [ ] `[history]` in `examples/config.toml` — `enabled` default **`true`**, `flush` **`true`**, `max_entries` **`5`** (§5.1).
- [ ] `libnotred/src/history/` (feature-gated): schema + migrations, `rusqlite` (bundled), `$XDG_CACHE_HOME/notred/history.db`.
- [ ] On startup: if `flush = true`, wipe DB before first `Notify`; if `flush = false`, skip (no timer).
- [ ] On `Notify`: insert (unless disabled); enforce cap (delete oldest when `N > 0`).
- [ ] States: `active` vs `closed` on timeout/app close; row retained until `remove` or cap eviction.
- [ ] IPC: `list_history`, `remove`, `history_changed` event on `subscribe`.
- [ ] `notredctl remove` + IPC `remove`: FDN close if active + delete DB row (subscribers use ctl, not DB).
- [ ] Unit tests: cap=5 drops oldest; `enabled=false` no writes; `0` unbounded until remove; `flush=true` wipes on restart; `flush=false` keeps rows across restart; feature off → no `rusqlite` link.
- [ ] `cargo deny` allow entry for SQLite/rusqlite as needed.

**Verify:** send 6× `notify-send` with `max_entries = 5` → DB has 5 rows, oldest gone; restart `notred` with `flush=true` → DB empty; with `flush=false` → rows retained; `notredctl list-history` / `remove`.

### Phase 4 — `notred-tui` manager (requires `history`)

- [ ] Crate / binary depends on `notred`+`notredctl` with **`history`** feature; document if history disabled at runtime.
- [ ] Child process or periodic spawn: **`notredctl subscribe`**, parse JSON lines from stdout.
- [ ] ratatui: `notredctl list-history` for paint; **`d`** → `notredctl remove <id>`; activate via `notredctl activate` (arrows-first keys).
- [ ] Dogfood: receive notifications while TUI **closed**, then open TUI and see full retained set.

**Verify:** full manager workflow with **only** `notred` + `notred-tui`; history survives TUI not running within same notred session.

### Phase 5 — release v0.1.0

- [ ] README: install, socket, IPC link, **not libnotify** disclaimer.
- [ ] systemd user unit `notred.service`.
- [ ] CHANGELOG; tag; publish `libnotred` if crates.io ready.

**Verify:** full §7 gates on release tag; dogfood `notify-send` + `notred-tui` on a real session.

---

## 9. Definition of done (platform v0)

- [ ] `notify-send` works with **only** `notred` running (no subscriber) — proves FDN.
- [ ] `notredctl reload|pause|close-all` works via IPC.
- [ ] **History (optional feature):** default build includes `history`; minimal build without; `[history] enabled`, `flush`, `max_entries` documented.
- [ ] `notred-tui`: browse session history when history enabled.
- [ ] **No** Wayland/Cairo in notred workspace.
- [ ] IPC v1 documented + golden tests.
- [ ] README states **not libnotify**.
- [ ] All §7 quality gates pass on `main` and release tags.

---

## 10. Open decisions (resolve in IPC.md before Phase 1)

1. **`update` payload:** full `body` every time vs `get(id)` on demand.
2. **`max_visible`:** enforced in notred queue (authoritative for `list` and popup subscribers) — separate from `[history].max_entries`.
3. **Icon delivery:** path string over IPC vs raw bytes vs SHM fd (v0: path or PNG bytes in `get_icon`).
4. **Pause behavior:** queue in notred vs drop `Notify` with full queue return code.
5. **Timeout vs history:** on auto-timeout, row stays `closed` in DB until `remove` or cap eviction (§5.1) — confirm in IPC.md.
6. **`reload` + `max_entries` change:** truncate to new cap immediately vs next `Notify` only.
7. **`reload` + `flush`:** does not flush mid-session; only startup honors `flush` (§5.1).

---

## 11. Document maintenance

Update this plan when:

- IPC `v` bumps or new `cmd` / `event.kind` added
- crate names or workspace layout change
- FDN surface changes
- CI / quality gate commands or feature-matrix rows change

---

## Revision history

| Date | Change |
| ---- | ------ |
| 2026-06-03 | Initial notred platform plan: IPC v1 sketch, crate boundaries, phased rollout |
| 2026-06-03 | §3: D-Bus + queue + IPC server in **`libnotred`**; **`notred`** binary thin |
| 2026-06-03 | §3.4: **notred-tui** — arrow keys primary; vim binds optional aliases only |
| 2026-06-03 | Client-agnostic; config/presentation out of scope for subscriber repos |
| 2026-06-03 | §8: phases numbered 0–5 (history = Phase 3) |
| 2026-06-03 | §5.1: history owned by notred (SQLite, flush, `max_entries`); Phase 3 dedicated |
| 2026-06-03 | §4.6: **notredctl** sole preferred connector; `notredctl subscribe` for streams |
| 2026-06-03 | §5.1: history **optional** — Cargo feature `history` + `[history] enabled` |
| 2026-06-03 | §5: subscriber configs external; history DB **`$XDG_CACHE_HOME/notred/history.db`** |
| 2026-06-03 | Restored after accidental deletion |
| 2026-06-03 | §7: quality gates (poshanka-aligned): fmt, typos, deny, clippy matrix, tests, doc; §7.1–7.2 test/CI discipline |
| 2026-06-03 | Phase 0 complete: `libnotred` wire + IPC server; `notredctl` ping/subscribe/list |
| 2026-06-03 | `libnotred` IPC-only in Phase 0; FDN/zbus land in Phase 1 |
