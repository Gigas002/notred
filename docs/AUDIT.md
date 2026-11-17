# Audit

A whole-application audit of notred, notredctl, notred-tui and libnotred
(2026-11-17) in five parts: bugs (§2), performance (§3), security and
robustness (§4), usability (§5), and code quality, tests and project hygiene
(§6). §7 lists what was left open, and §8 is the checklist to re-run the audit.

Each finding says what was found, how serious it is, and what was done:
**fixed** (with the commit), **accepted** (with the reason), or **OK**
(checked, nothing to do). Every fix is its own commit, with a regression test
where one can be written.

Severity: **high** (lost notifications or a crash in normal use), **medium**
(wrong behaviour in a reachable case, an ignored setting), **low** (hardening,
polish, or needs an unusual setup).

## 1. Method

Every source file was read in full and checked against the Desktop
Notifications spec 1.2. Each finding was then reproduced with the release
build on a private `dbus-daemon` session bus with its own runtime, config and
cache directories, so the desktop's notification daemon was never touched.
Notifications were sent with `notify-send`, `gdbus` and a small Gio script
(for `image-data` avatars); signals were read with `gdbus monitor`; the IPC
socket was driven with `notredctl` and raw requests. poshanka's notred script
and click handling were read to judge how findings reach the popup.

Builds checked: default (the AUR package's configuration), and
`--all-features` (history). Static checks: `cargo clippy` at the three gate
levels, `cargo deny check`, `typos`, `cargo llvm-cov`.

## 2. Bugs

| Area | Finding | Status |
| ---- | ------- | ------ |
| Click without a default action | `activate` without a key always invoked `"default"`. Chat apps often offer only named actions (`reply`), so poshanka's left click (that call) failed with `unknown action key "default"` and the notification could not be clicked away; `button_left`/`touch` input likewise. Now it runs `default` if offered, else dismisses. | Fixed, high (9da578a) |
| Close after an action | After `ActionInvoked` the notification stayed. The spec's `resident` hint is what keeps one open after an action; without it servers remove it. Now closed (reason 2) after the signal, unless resident. | Fixed, medium (e446d23) |
| Pause loses notifications | Held notifications' expiry timers ran while paused: four 1 s notifications during 1.5 s of pause left nothing to show on unpause (and senders got "expired"). Timers now start when unpausing shows them. | Fixed, high (a892a57) |
| Unpause and `max_visible` | Unpausing showed every held notification, ignoring `[queue].max_visible`. | Fixed, low (a851dd1) |
| Kept history overwritten | Ids restart at 1 each run and history rows are keyed by id, so with `flush = false` new notifications replaced old rows one by one. Ids now continue after the highest stored one. | Fixed, medium (28cee25) |
| TUI and image icons | notred-tui's parser knew only icon names and paths, so one notification with an `image-data` avatar made the whole history list fail. | Fixed, medium (2d624f8) |
| Silent config mistakes | Unknown keys (`[queue] default_timeout`) were ignored without a word. A broken override fragment was logged before logging was set up (so never shown) and dropped every fragment; `reload` then still said "reloaded". All are reported now; a broken reload keeps the running config. | Fixed, medium (b02d28b) |
| Fragment include cycle | A fragment whose `[paths].overrides` led back to itself crashed notred with a stack overflow at start-up. Now a config error. | Fixed, low (6e4a212) |
| SIGTERM | No signal handling: `systemctl --user stop` or Ctrl+C killed notred and left `notred.sock` behind. Now a clean exit that removes the socket. | Fixed, low (09619d0) |
| Malformed IPC requests | Outside `subscribe`, a bad line ended the connection without a reply. Now `INVALID_REQUEST`, connection kept. | Fixed, low (f65642d) |
| Client error messages | `list` and `ping` dropped the daemon's error message. | Fixed, low (17e10fc) |
| `on_action` env | Lacked `NOTRED_SUMMARY`, which the example config lists for hooks. | Fixed, low (f5ac5dc) |
| notred-tui settings | `--ctl` lost to `ctl` in `tui.toml` (the reverse of CLI > config), and a missing `--config` file silently meant defaults. | Fixed, low (7ffd624, 9f3515c) |
| `CloseNotification` | Emits exactly one `NotificationClosed` (reason 3). | OK |
| `replaces_id`, expiry, `close_all` | Checked end to end on the bus. | OK |
| Second instance | Refused before it touches the running daemon's socket (IPC ping, then the bus name). | OK |

## 3. Performance

| Area | Finding | Status |
| ---- | ------- | ------ |
| `update` events with avatars | Every `update` re-sends all active notifications with their base64 `image-data`. 20 notifications with 256 px avatars: one update line is 7 MB and the stream reached 71 MB, which poshanka parses in full each time; it grows with the square of the count. Needs an IPC change agreed with poshanka. | Open (§7) |
| `Notify` with `image-data` | ~25 ms per 256 px avatar (2 ms at 64 px), mostly zbus decoding `ay` into one variant per byte. | Accepted |
| Idle | No timers or polling when nothing happens; RSS 15 MB. | OK |

## 4. Security and robustness

| Area | Finding | Status |
| ---- | ------- | ------ |
| `image-data` validation | Any process on the session bus can notify; its pixel buffer was forwarded unchecked although `docs/IPC.md` tells subscribers to index it by `rowstride`. Short buffers, a rowstride below one row, one channel or 16-bit samples now drop the icon. poshanka's decoder panics on a one-channel image, so this protected it too. | Fixed, medium (14f876f) |
| History file mode | `history.db` (notification text, login codes) was created 0644. Now 0600; SQLite's journals follow. | Fixed, low (cbf2db5) |
| IPC socket | 0600 in `$XDG_RUNTIME_DIR`; same-user only, no authentication, as documented. | OK |
| Hooks | Run as argv, never through a shell by notred; notification text reaches hooks only as environment variables. | OK |
| `/tmp` socket fallback | Without `XDG_RUNTIME_DIR` notred binds `/tmp/notred.sock` (notredctl refuses that case). Another user can only block it, not hijack it. | Accepted |
| IPC line length | A client may send an unbounded line; same-user only. | Accepted |
| Dependencies | `cargo deny check` passes; lockfile current. | OK |

## 5. Usability

| Area | Finding | Status |
| ---- | ------- | ------ |
| notred-tui in default builds | The TUI needs the `history` feature, which a default build (and the AUR package) lacks; it then exits with an explanation. README now says how to build it. | Fixed, docs (b197889) |
| Docs | `docs/IPC.md` and the example config pointed at a missing `PLAN.md` and spoke in development phases. | Fixed (b197889) |

## 6. Code quality, tests and hygiene

| Area | Finding | Status |
| ---- | ------- | ------ |
| Untested daemon | The D-Bus host, IPC client, daemon binary and notredctl had no tests. `notred/tests/live.rs` runs the real daemon on a private bus (CI installs `dbus-daemon`, `NOTRED_LIVE_TESTS=1`); `notredctl/tests/cli.rs` runs the CLI against an in-process server; the TUI is rendered with `TestBackend` and driven through a fake notredctl. | Fixed (f0ccc7f, df90946, 20ac052) |
| Coverage | 52 % → 80 % of lines (`cargo llvm-cov --workspace --all-features` with `dbus-daemon` installed). | Fixed |
| Cargo warnings | Three manifest warnings on every build. | Fixed (72b9cc7) |
| Gates | fmt, typos, deny, clippy and tests at the three feature levels, and docs, pass on every commit. | OK |

## 7. Left open

- **Avatar-heavy update stream (§3).** Options: send each icon once and refer
  to it by id afterwards, or scale `image-data` down at ingestion. Either
  changes what poshanka receives, so it belongs with poshanka's audit.
- **poshanka's raw icon decoder** reads past the buffer for fewer than three
  channels; notred no longer sends such icons, but poshanka should check too.
- **Pause and hooks.** `on_notify` still runs for notifications held while
  paused (do not disturb); whether it should is a policy choice.
- **`transient` and other hints** (`sound-file`, `suppress-sound`, `x`/`y`)
  are ignored; nothing in the current setup uses them.
- **Unknown protocol versions.** A request with `"v": 2` is accepted as v1.

## 8. Re-running the audit

```sh
# Gates (docs/ARCHITECTURE.md §8); live tests need dbus-daemon
cargo fmt --all -- --check && typos && cargo deny check
for f in --no-default-features "" --all-features; do
  cargo clippy --workspace --all-targets $f -- -D warnings
  NOTRED_LIVE_TESTS=1 cargo test --workspace $f
done
cargo doc --workspace --no-deps

# Coverage (system LLVM tools on Arch: LLVM_COV / LLVM_PROFDATA)
NOTRED_LIVE_TESTS=1 LLVM_COV=/usr/bin/llvm-cov LLVM_PROFDATA=/usr/bin/llvm-profdata \
  cargo llvm-cov --workspace --all-features --summary-only
```

To try a change by hand without touching the desktop's daemon, start a
private bus (`dbus-daemon --session --print-address`), point
`DBUS_SESSION_BUS_ADDRESS`, `XDG_RUNTIME_DIR` (a short path: socket paths are
limited to 108 bytes) and `XDG_CONFIG_HOME` at private values, then run
`notred` and send with `notify-send`.
