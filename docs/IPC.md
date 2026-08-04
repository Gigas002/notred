# notred IPC v1 (Phase 3)

## Transport

- **Socket:** Unix domain stream, default `$XDG_RUNTIME_DIR/notred.sock` (overridable in `notred.toml`)
- **Framing:** newline-delimited JSON (NDJSON) — one JSON object per line, UTF-8
- **Version:** every line includes `"v": 1`

## Audience

- **Integrators:** use **`notredctl`** (`notredctl --help`, JSON on stdout) — see [`PLAN.md`](PLAN.md) §4.6.
- **Implementors:** this document + `examples/ipc-examples/*.jsonl` golden fixtures.

Phase 3 adds **`list_history`** and **`remove`** plus the **`history_changed`** subscribe event (`history` Cargo feature + `[history] enabled` at runtime).

---

## Requests

```
{"v":1,"cmd":"<command>"[, ...args]}
```

| `cmd`           | Extra fields                                              | Phase |
| --------------- | --------------------------------------------------------- | ----- |
| `ping`          | —                                                         | 0     |
| `subscribe`     | —                                                         | 0     |
| `list`          | —                                                         | 0     |
| `dismiss`       | `"id": u32`                                               | 1     |
| `close_all`     | —                                                         | 1     |
| `activate`      | `"id": u32`, `"key": str` (optional)                      | 2     |
| `reload`        | —                                                         | 2     |
| `pause`         | —                                                         | 2     |
| `unpause`       | —                                                         | 2     |
| `list_history`  | `active_only`, `app_id`, `since` (all optional)           | 3     |
| `remove`        | `"id": u32`                                               | 3     |
| `input`         | `"id": u32`, `"event_kind": string`                       | 6     |

### `input` event kinds

Subscribers (e.g. poshanka) report pointer gestures with **`event_kind`** — no other aliases on the wire:

| `event_kind`     | `[events]` hook      | Default when hook absent |
| ---------------- | -------------------- | ------------------------ |
| `button_left`    | `on_button_left`     | no actions → dismiss; has actions → `activate` default key |
| `button_middle`  | `on_button_middle`   | dismiss |
| `button_right`   | `on_button_right`    | dismiss |
| `touch`          | `on_touch`           | same as `button_left` |

When a configured hook exists for the kind, **only** the shell hook runs (no automatic dismiss/activate). Hooks receive env: `NOTRED_ID`, `NOTRED_APP_ID`, `NOTRED_SUMMARY`, `NOTRED_EVENT_KIND`.

`activate` / `close` (dismiss) remain separate shortcuts for whole-card tap (poshanka v0) and **notred-tui** — they do not use `input`.

### `activate` action keys

When `"key"` is omitted, the server uses **`"default"`**.

### History availability

`list_history` and `remove` return `NOT_IMPLEMENTED` when the daemon is built without the `history` feature or when `[history] enabled = false`.

---

## Responses

### Success

| `type`    | Fields                         | Reply to |
| --------- | ------------------------------ | -------- |
| `pong`    | —                              | `ping`   |
| `ok`      | —                              | mutating commands |
| `items`   | `"items": MinimalNotification[]` | `list` |
| `history` | `"rows": HistoryRow[]`         | `list_history` |
| `event`   | `"event": Event`               | `subscribe` stream |

### `MinimalNotification`

| Field            | Type                     | Notes |
| ---------------- | ------------------------ | ----- |
| `id`              | u32                      | |
| `app_id`          | string                   | |
| `summary`         | string                   | |
| `body`            | string                   | Pango markup when `body_markup` is `true` (sender-supplied; validate before rendering as markup) |
| `urgency`         | `"low"` \| `"normal"` \| `"critical"` | |
| `timeout_ms`      | i32                      | `-1` = daemon default, `0` = persistent |
| `icon`            | `Icon`, optional         | omitted when the sender provided no icon — see below |
| `has_actions`     | bool                     | |
| `timestamp`       | i64, optional            | Unix seconds |
| `value`           | i32, optional            | FDN `value` hint (progress percent, `0..=100`); omitted when unset or out of range |
| `category`        | string, optional         | FDN `category` hint (e.g. `"email.arrived"`) |
| `desktop_entry`   | string, optional         | FDN `desktop-entry` hint (desktop file id, no `.desktop` suffix) |
| `body_markup`     | bool                     | Snapshot of `[notifications].body_markup` at ingestion time — always present |

`[notifications].body_markup` (`notred.toml`, default `true`) also controls whether
the FDN `GetCapabilities` D-Bus call advertises `"body-markup"` to senders.

### `icon`

`MinimalNotification.icon` / `HistoryRow.icon` (both optional — omitted when the
sender provided no icon). One of three untagged shapes, tried in this order by
`icon_from_image_data` → `icon_from_hints` → `icon_from_str` (`app_icon`):

| Shape  | Fields | Source |
| ------ | ------ | ------ |
| Name   | `"name": string` | FDN `app_icon` or `image-path`/`image_path` hint, no `/` |
| Path   | `"path": string` | FDN `app_icon` or `image-path`/`image_path` hint, containing `/` (`file://` prefix stripped) |
| Raw    | `"width"`, `"height"`, `"rowstride"`, `"has_alpha"`, `"bits_per_sample"`, `"channels"` (all i32/bool), `"data"` (base64 string) | FDN `image-data`/`image_data`/`icon_data` hint (`(iiibiiay)`) — raw pixel buffer, used by senders with no icon-theme name or on-disk file (e.g. chat app avatars) |

`data` is the raw pixel bytes, row-major, straight (non-premultiplied) alpha,
base64-encoded for JSON transport. `rowstride` may exceed `width * channels`
(padding) — consumers must index rows by `rowstride`, not `width * channels`.

### `HistoryRow`

Same core fields as `MinimalNotification`, plus:

| Field         | Type                          |
| ------------- | ----------------------------- |
| `action_keys` | `string[]`                    |
| `received_at` | i64 (Unix seconds)            |
| `state`       | `"active"` \| `"closed"`      |

### `Event` kinds

| `kind`            | When |
| ----------------- | ---- |
| `update`          | Active queue changed |
| `reload`          | Config reloaded |
| `history_changed` | History DB mutated |

---

## Golden examples

| File                                         | Commands        |
| -------------------------------------------- | --------------- |
| `examples/ipc-examples/list_history.jsonl`   | `list_history`  |
| `examples/ipc-examples/remove.jsonl`         | `remove`        |
| `examples/ipc-examples/input.jsonl`          | `input`         |

See also Phase 0–2 fixtures (`ping`, `list`, `subscribe`, `activate`, `reload`, `pause`).

---

## Security

- Socket lives under `$XDG_RUNTIME_DIR` with mode `0600` on create.
- No authentication in v0 (session user only).
