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

See also Phase 0–2 fixtures (`ping`, `list`, `subscribe`, `activate`, `reload`, `pause`).

---

## Security

- Socket lives under `$XDG_RUNTIME_DIR` with mode `0600` on create.
- No authentication in v0 (session user only).
