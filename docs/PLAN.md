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

### 1.4 abar / trayd analogue (required separation)

**notred** + **poshanka** must coexist the same way **trayd** + **abar** do: one repo owns state and policy; the other only paints and talks through a **connector CLI**. No Cargo dependency between them; either side is swappable in principle.

| Layer | trayd stack | notred stack |
| ----- | ----------- | ------------ |
| Daemon (state + policy) | **trayd** — SNI host, icon/menu cache | **notred** — FDN, queue, timeouts, `[events]` |
| Connector CLI | **trayctl** | **notredctl** |
| Graphical UI | **abar** — draws tray icons | **poshanka** — draws notification cards |
| Terminal client | **tray-tui** | **notred-tui** |
| State stream | `trayctl subscribe` → NDJSON on stdout | `notredctl subscribe` → NDJSON on stdout |
| Reconnect wrapper | `abar/examples/scripts/tray/tray.sh` | `poshanka/examples/scripts/notred-subscribe.sh` |
| UI → daemon mutations | `trayctl menu`, `trayctl activate`, … | `notredctl close`, `activate`, **`input`**, … |

**Hard rules (same spirit as trayd `docs/IPC.md` + abar `docs/EXEC.md`):**

- External UIs **must not** link `libnotred` or open `notred.sock` in the supported design — only **`notredctl`** (or scripts that wrap it).
- **notred** **never** draws pixels; **poshanka** **never** owns the FDN bus name, queue, or timeout engine.
- Wire types may be duplicated locally in subscriber repos (abar copies `MinimalTrayItem`; poshanka copies `MinimalNotification` shapes).

**Deliberate difference — where click / gesture hooks live:**

| Concern | abar + trayd | notred + poshanka |
| ------- | ------------ | ----------------- |
| Who runs shell on user click? | **abar** — `[tray].on_left_click` spawns user scripts (`tray-menu.sh`) | **notred** — `[events].on_button_left` etc. in **notred** config |
| What does the UI send? | abar spawns hook directly; trayd sees `trayctl activate` only from menu scripts | poshanka sends **`notredctl input <id> <event_kind>`** (or v0 shortcuts `activate` / `close`) — **never** hook argv |
| Why | Tray click behavior is bar-specific (picker, rofi, …) | Notification behavior is **session policy** (dismiss, activate, per-app overrides) — must be identical for **poshanka**, **notred-tui**, and future subscribers |

**poshanka reports `id` + `event_kind` only.** notred resolves notification context, merges override fragments, runs `[events]` hooks, and emits FDN signals. This is **not** optional for per-button mako/dunst parity — see [Phase 6](#phase-6--subscriber-input-events--events-hooks).

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
| `history/` | Session notification **log** (SQLite) — **`history` feature only**; see §5.3 |

**Cargo features:**

| Feature | Enables | Used by |
| ------- | ------- | ------- |
| `server` (default for `notred` bin) | `dbus/`, `queue/`, `timeouts/`, `spawn/`, `ipc/server/`, `host/`, `wire/` | **`notred`** binary |
| `history` | `history/`, `rusqlite`; IPC `list_history`, `remove`, `history_changed`; ctl subcmds | Full install, **notred-tui** |

Default **`notred`** binary: `default = []` (no optional features). Enable history: `cargo build -p notred --features history`. Minimal daemon: `cargo build -p notred` (no SQLite, no history RPCs) — §5.3.

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
| `notredctl input <id> <event_kind>` | Report raw pointer gesture; notred runs matching `[events]` hook — [Phase 6](#phase-6--subscriber-input-events--events-hooks) |

Post-v0: modes (mako-style DND) if needed.

**`input` event kinds (v1):** `button_left`, `button_middle`, `button_right`, `touch`. Subscribers (poshanka) map Wayland seat events to these strings only — no `left_button_click` or other aliases in the wire protocol.

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
| `input` | `{ "id": u32, "event_kind": string }` — subscriber reports pointer gesture; notred runs `[events]` hook — [Phase 6](#phase-6--subscriber-input-events--events-hooks) |

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

### 4.4 Subscriber input events (poshanka / graphical UIs)

Graphical subscribers **do not** run `[events]` shell hooks and **do not** embed dismiss/activate policy. They report user gestures; **notred** is the sole executor of behavior + FDN.

```text
poshanka (Wayland pointer)
    │  notredctl input 42 button_left
    ▼
notredctl
    ▼
notred — resolve id, merge override fragments, match [events]
    │  optional shell: on_button_left, on_button_middle, …
    │  optional queue mutation (dismiss / activate per hook outcome)
    │  ActionInvoked / NotificationClosed on D-Bus
    ▼
Apps
```

**v0 convenience shortcuts** (whole-card tap, no per-button hit test): subscribers may call `notredctl activate <id>` or `notredctl close <id>` directly — same as **notred-tui** calling `activate` / `remove`. These bypass `input` but still execute notred-side policy (`has_actions`, action keys). Document precedence in IPC.md: when `input` exists, per-button gestures **must** use `input`; whole-card primary tap may keep using `activate` / `close` until poshanka ships button hit regions.

**Precedence (document in IPC.md):**

1. `input` with `event_kind` → matching `on_button_*` / `on_touch` from merged config (override fragments win per §5.2).
2. If no hook configured for that kind → default policy (e.g. `button_left` + no actions → dismiss; + `has_actions` → activate default key).
3. `activate` / `close` RPCs → explicit semantic actions (TUI, v0 poshanka whole-card tap); still run `on_action` where applicable for `activate`.

**Env vars for hooks** (extend `on_action` pattern): `NOTRED_ID`, `NOTRED_APP_ID`, `NOTRED_EVENT_KIND`, `NOTRED_ACTION_KEY` (when activate implied).

### 4.5 Security

- Socket under `$XDG_RUNTIME_DIR` only; mode `0600` on create.
- No authentication in v0 (session user only).
- Document threat model: local user can dismiss/spoof manager; not for multi-user machines.

### 4.6 Why not D-Bus for external UIs?

- FDN is the **app-facing** API. External tools use **`notredctl`**, not a second D-Bus surface and **not** raw socket by default.
- The socket exists so **one** connector (`notredctl`) can be tested and versioned; protocol details stay in **IPC.md** for ctl/daemon authors.

### 4.7 Subscriber integration (`notredctl` only — preferred)

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

### 5.2 `[events]` hooks and override fragments (poshanka-parity layout)

**Owner:** notred only. Subscribers never parse or execute these — they call `notredctl input` / `activate` / `close`.

**Same fragment model as poshanka** — parallel directory trees under `$XDG_CONFIG_HOME/notred/` (or `examples/` in-repo). poshanka uses fragments for **pixels** (`theme.toml`); notred uses the **same `[override]` + `[paths].overrides` shape** for **behavior** (`[events]`). A user can mirror paths:

```text
poshanka/config.toml          notred/notred.toml
poshanka/apps/some_app/…      notred/apps/some_app/…     ← per-app behavior
poshanka/urgency/critical/…   notred/urgency/critical/…  ← global urgency behavior
```

#### Global handlers (`examples/config.toml`)

Every hook may be set once in the root config. Omitted keys fall through to built-in default policy (§5.2.3).

| Key | When fired | Env vars (representative) |
| --- | ---------- | ------------------------- |
| `on_action` | `notredctl activate` / action key chosen | `NOTRED_ID`, `NOTRED_ACTION_KEY`, `NOTRED_APP_ID` |
| `on_button_left` | `notredctl input … button_left` | + `NOTRED_EVENT_KIND` |
| `on_button_middle` | `… button_middle` | same |
| `on_button_right` | `… button_right` | same |
| `on_touch` | `… touch` | same |
| `on_notify` | new `Notify` accepted into queue | `NOTRED_ID`, `NOTRED_APP_ID`, `NOTRED_SUMMARY`, … |

```toml
[paths]
overrides = [
    "urgency/low/config.toml",
    "urgency/critical/config.toml",
    "apps/some_app/config.toml",
]

[events]
# Global defaults — any key omitted here can still be set in a fragment
on_button_left   = ["sh", "-c", "echo default left $NOTRED_ID"]
on_button_middle = ["sh", "-c", "echo default middle"]
on_button_right  = ["sh", "-c", "echo default right"]
on_touch         = ["sh", "-c", "echo default touch"]
on_notify        = ["sh", "-c", "echo new notification $NOTRED_APP_ID"]
on_action        = ["sh", "-c", "echo activated $NOTRED_ACTION_KEY"]
```

#### Override fragments (`[paths].overrides`)

Each entry is a **separate TOML file** with a top-level `[override]` table (same schema as poshanka fragments — **not** `[[override]]` arrays in one file).

| `override.type` | Required field | Matches notification |
| --------------- | -------------- | -------------------- |
| `app` | `name` (→ `app_id`) | `app_id` from queue / IPC |
| `urgency` | `level` (`low` \| `normal` \| `critical`) | `urgency` field |

**App fragments** may nest urgency sub-fragments via their own `[paths].overrides` (only valid inside `type = "app"`):

```toml
# examples/apps/some_app/config.toml
[override]
type = "app"
name = "some_app"

[paths]
overrides = [
    "urgency/low/config.toml",
    "urgency/critical/config.toml",
]

[events]
on_button_left = ["sh", "-c", "echo some_app left $NOTRED_ID"]
```

```toml
# examples/apps/some_app/urgency/critical/config.toml
[override]
type = "urgency"
level = "critical"

[events]
on_notify = ["sh", "-c", "echo critical some_app notify"]
```

```toml
# examples/urgency/critical/config.toml — global urgency (all apps)
[override]
type = "urgency"
level = "critical"

[events]
on_button_right = ["sh", "-c", "echo global critical right dismiss"]
```

#### 5.2.1 Merge order (field-by-field)

When resolving which argv to run for a hook, merge **`[events]` tables** in this order — **later layers override earlier layers per key only** (unset keys inherit):

```text
root notred.toml [events]
  → matching global urgency fragment
  → matching app fragment
  → matching app nested urgency fragment
```

Same precedence as poshanka `resolve_layers` / `apply_layers` (base → base_urgency → app → app_urgency). Implement in `notred/src/config/` (port poshanka's `load_overrides` + `resolve_layers` pattern; merge `[events]` instead of theme).

**Example:** root sets `on_button_left` + `on_notify`; `apps/firefox` sets only `on_button_left`; firefox critical nested sets only `on_notify` → firefox critical notification uses: `on_button_left` from firefox app fragment, `on_notify` from firefox/critical, `on_button_right` still from root.

#### 5.2.2 Hook lookup at runtime

On `input`, `activate`, or `Notify`:

1. Build `OverrideLayers` from notification `app_id` + `urgency`.
2. Merge `[events]` per §5.2.1.
3. Pick argv for the fired hook key (`on_button_left`, …).
4. If argv present → spawn hook (non-blocking, `libnotred/src/spawn/`).
5. If absent → §5.2.3 default policy.

All six hooks (`on_action`, `on_button_left`, `on_button_middle`, `on_button_right`, `on_touch`, `on_notify`) participate in the **same** override merge — no hook is “global only”.

#### 5.2.3 Default policy when no hook matches

- `button_left` on notification without actions → dismiss (`NotificationClosed` user).
- `button_left` with `has_actions` → activate default action key (same as `notredctl activate <id>`).
- Middle/right → no-op or dismiss per mako/dunst conventions (pick one, document).
- `on_notify` absent → no extra shell; queue + IPC `update` only.

**Visual** theme overrides remain in **poshanka** only. **Behavior** fragments live only under **notred** config paths.

### 5.3 Notification history (optional)

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
enabled = false

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
| **`enabled`** | **`false`** (default when `history` feature compiled): set **`true`** to enable SQLite + history IPC. When **`false`**: no DB writes; `notredctl list-history` / `remove` error or no-op. Ignored if binary lacks `history` feature. |
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

**Non-goals (v0):** inhibition, inline `image-data` body, daemon sound — unless explicitly added later. (Optional history §5.3; **notred-tui** requires history enabled.)

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

- [x] Cargo feature **`history`** on `libnotred` / `notred` / `notredctl` (default **off**; opt in with `--features history`).
- [x] `[history]` in `examples/config.toml` — `enabled` default **`false`**, `flush` **`true`**, `max_entries` **`5`** (§5.3).
- [x] `libnotred/src/history/` (feature-gated): schema + migrations, `rusqlite` (bundled), `$XDG_CACHE_HOME/notred/history.db`.
- [x] On startup: if `flush = true`, wipe DB before first `Notify`; if `flush = false`, skip (no timer).
- [x] On `Notify`: insert (unless disabled); enforce cap (delete oldest when `N > 0`).
- [x] States: `active` vs `closed` on timeout/app close; row retained until `remove` or cap eviction.
- [x] IPC: `list_history`, `remove`, `history_changed` event on `subscribe`.
- [x] `notredctl remove` + IPC `remove`: FDN close if active + delete DB row (subscribers use ctl, not DB).
- [x] Unit tests: cap=5 drops oldest; `enabled=false` no writes; `0` unbounded until remove; `flush=true` wipes on restart; `flush=false` keeps rows across restart; feature off → no `rusqlite` link.
- [x] `cargo deny` allow entry for SQLite/rusqlite as needed.

**Verify:** send 6× `notify-send` with `max_entries = 5` → DB has 5 rows, oldest gone; restart `notred` with `flush=true` → DB empty; with `flush=false` → rows retained; `notredctl list-history` / `remove`.

### Phase 4 — `notred-tui` manager (requires `history`)

- [x] Crate / binary depends on `notred`+`notredctl` with **`history`** feature; document if history disabled at runtime.
- [x] Child process or periodic spawn: **`notredctl subscribe`**, parse JSON lines from stdout.
- [x] ratatui: `notredctl list-history` for paint; **`d`** → `notredctl remove <id>`; activate via `notredctl activate` (arrows-first keys).
- [x] Dogfood: receive notifications while TUI **closed**, then open TUI and see full retained set.

**Verify:** full manager workflow with **only** `notred` + `notred-tui`; history survives TUI not running within same notred session.

### Phase 5 — Queue policy (timeouts, max_visible, overrides) ✅

Platform gaps needed before graphical subscribers match mako/dunst expectations. **Does not** include pointer `input` — see Phase 6.

- [x] `libnotred/src/timeouts/` — Tokio timers from `timeout_ms`; auto-dismiss → `NotificationClosed` (reason timeout).
- [x] `max_visible` in `examples/config.toml` — cap active queue; `list` / `update` snapshots respect cap (separate from `[history].max_entries`).
- [x] `paths.overrides` loader in `notred/src/config/` — **poshanka-parity** fragments: `[override]` metadata, nested app→urgency, field-by-field `[events]` merge (§5.2.1).
- [x] Example tree: `examples/urgency/*/config.toml`, `examples/apps/some_app/**` (behavior only, no theme).
- [x] `reload` re-applies overrides without restart.
- [x] Unit tests: timeout fires; cap evicts oldest from active set; override merge order.

**Verify:** `notify-send` with short timeout auto-closes; seventh notification with `max_visible = 6` drops oldest from `notredctl list`; per-app `on_button_left` override visible in merged config dump (debug subcommand or test helper).

### Phase 6 — Subscriber input events + `[events]` hooks ✅

**Required for poshanka per-button gestures** ([poshanka PLAN §5.6](https://github.com/Gigas002/poshanka/blob/master/docs/PLAN.md#56-upstream-todo--notred--notredctl)). Implements the abar/trayd-style split for **behavior**: UI reports gestures; daemon runs hooks.

- [x] IPC `cmd: "input"` with `{ "id", "event_kind" }` in `libnotred/src/wire/`.
- [x] `notredctl input <id> <event_kind>` in `notredctl/src/cli/mod.rs`.
- [x] Daemon handler: resolve notification, merge §5.2 overrides, spawn matching hook via `libnotred/src/spawn/`.
- [x] Extend `EventsConfig` + `examples/config.toml`: all six hooks; global `[events]` + per-app / per-urgency fragments (§5.2).
- [x] `resolve_events_for(notification)` — merge override layers before spawn (port poshanka `resolve_layers` semantics).
- [x] Default policy when hook absent (§5.2).
- [x] FDN side effects: correct `NotificationClosed` / `ActionInvoked` / dismiss reason when hook or default policy implies dismiss or activate.
- [x] Document event kinds, precedence vs `activate`/`close`, env vars in `docs/IPC.md`.
- [x] Golden fixtures: `examples/ipc-examples/input-*.jsonl`.

**Verify:** `notredctl input 1 button_left` with configured hook runs shell; `dbus-monitor` shows expected signals; poshanka (or fake client) can drive full gesture path without poshanka reading notred config.

### Phase 7 — release v0.1.0

- [x] README: install, socket, IPC link, **not libnotify** disclaimer.
- [x] systemd user unit example in-repo (`examples/notred.service`); install target **`~/.config/systemd/user/notred.service`** — document `systemctl --user enable --now notred.service`.
- [x] CHANGELOG ([`CHANGELOG.md`](CHANGELOG.md)).
- [ ] Tag `v0.1.0` and push (triggers [Deploy](.github/workflows/deploy.yml) — see README **Releasing**).
- [ ] Publish `libnotred` to crates.io (`CARGO_REGISTRY_TOKEN` secret; automated on tag push when configured).

**Verify:** full §7 gates on release tag; dogfood `notify-send` + `notred-tui` on a real session.

---

## 9. Definition of done (platform v0)

- [ ] `notify-send` works with **only** `notred` running (no subscriber) — proves FDN.
- [ ] `notredctl reload|pause|close-all` works via IPC.
- [x] **History (optional feature):** opt-in Cargo feature `history`; `[history] enabled` defaults **off**; `flush`, `max_entries` documented (README + `examples/config.toml`).
- [ ] `notred-tui`: browse session history when history enabled.
- [x] **`notredctl input`** + `[events].on_button_*` — graphical subscribers can delegate all pointer policy to notred ([Phase 6](#phase-6--subscriber-input-events--events-hooks)).
- [ ] **Timeouts** + **`max_visible`** — active queue behaves like a real notification daemon ([Phase 5](#phase-5--queue-policy-timeouts-max_visible-overrides)).
- [ ] **No** Wayland/Cairo in notred workspace.
- [ ] IPC v1 documented + golden tests.
- [x] README states **not libnotify**.
- [ ] All §7 quality gates pass on `main` and release tags.

---

## 10. Open decisions (resolve in IPC.md before Phase 1)

1. **`update` payload:** full `body` every time vs `get(id)` on demand.
2. **`max_visible`:** enforced in notred queue (authoritative for `list` and popup subscribers) — separate from `[history].max_entries`.
3. **Icon delivery:** path string over IPC vs raw bytes vs SHM fd (v0: path or PNG bytes in `get_icon`).
4. **Pause behavior:** queue in notred vs drop `Notify` with full queue return code.
5. **Timeout vs history:** on auto-timeout, row stays `closed` in DB until `remove` or cap eviction (§5.3) — confirm in IPC.md.
6. **`reload` + `max_entries` change:** truncate to new cap immediately vs next `Notify` only.
7. **`reload` + `flush`:** does not flush mid-session; only startup honors `flush` (§5.3).

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
| 2026-07-03 | §5.2: poshanka-parity override tree — global + per-app `[events]`, nested urgency, field merge |
| 2026-07-03 | §4.4: subscriber `input` flow; §5.2 `[events]` + overrides; Phase 5–7 (policy, input, release) |
| 2026-06-03 | `libnotred` IPC-only in Phase 0; FDN/zbus land in Phase 1 |
