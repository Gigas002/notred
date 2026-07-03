# notred IPC v1 (Phase 2)

## Transport

- **Socket:** Unix domain stream, default `$XDG_RUNTIME_DIR/notred.sock` (overridable in `notred.toml`)
- **Framing:** newline-delimited JSON (NDJSON) — one JSON object per line, UTF-8
- **Version:** every line includes `"v": 1`

## Audience

- **Integrators:** use **`notredctl`** (`notredctl --help`, JSON on stdout) — see [`PLAN.md`](PLAN.md) §4.6.
- **Implementors:** this document + `examples/ipc-examples/*.jsonl` golden fixtures.

Phase 2 implements **`ping`**, **`subscribe`**, **`list`**, **`dismiss`**, **`close_all`**, **`activate`**, **`reload`**, **`pause`**, and **`unpause`**. History commands land in Phase 3 ([`PLAN.md`](PLAN.md) §4.1).

---

## Requests

```
{"v":1,"cmd":"<command>"[, ...args]}
```

| `cmd`        | Extra fields              | Phase |
| ------------ | ------------------------- | ----- |
| `ping`       | —                         | 0     |
| `subscribe`  | —                         | 0     |
| `list`       | —                         | 0     |
| `dismiss`    | `"id": u32`               | 1     |
| `close_all`  | —                         | 1     |
| `activate`   | `"id": u32`, `"key": str` (optional) | 2 |
| `reload`     | —                         | 2     |
| `pause`      | —                         | 2     |
| `unpause`    | —                         | 2     |
| `get`, …     | TBD                       | 3+    |

### `activate` action keys

When `"key"` is omitted, the server uses **`"default"`** — the conventional Freedesktop action key for the primary action.

If the notification advertises actions (`has_actions: true`), the key must match one of the action keys from the original `Notify` call. Notifications without actions accept only `"default"`.

On success, notred emits D-Bus **`ActionInvoked`** and may run the optional `[events].on_action` hook from config (see `examples/config.toml`).

---

## Responses

### Success

```
{"v":1,"type":"<type>"[, ...fields]}
```

| `type`  | Fields                           | Reply to                                      |
| ------- | -------------------------------- | --------------------------------------------- |
| `pong`  | —                                | `ping`                                        |
| `ok`    | —                                | `dismiss`, `close_all`, `activate`, `reload`, `pause`, `unpause` |
| `items` | `"items": MinimalNotification[]` | `list`                                        |
| `event` | `"event": Event`                 | `subscribe` (stream)                          |

### Error

```
{"v":1,"error":{"code":"<CODE>","message":"..."}}
```

| `code`            | Meaning                           |
| ----------------- | --------------------------------- |
| `NOT_FOUND`       | id / resource missing             |
| `NOT_IMPLEMENTED` | command not available (e.g. `reload` without config path) |
| `INVALID_REQUEST` | malformed JSON, unknown action key, or protocol misuse |

---

## Types

### `MinimalNotification`

```json
{
  "id": 1,
  "app_id": "org.example.App",
  "summary": "Title",
  "body": "Body text",
  "urgency": "normal",
  "timeout_ms": 5000,
  "icon": { "name": "dialog-information" },
  "has_actions": false,
  "timestamp": 1717430400
}
```

| Field         | Type                                                   | Notes                                   |
| ------------- | ------------------------------------------------------ | --------------------------------------- |
| `id`          | u32                                                    | Server-assigned notification id         |
| `app_id`      | string                                                 | Freedesktop application id              |
| `summary`     | string                                                 | Short title                             |
| `body`        | string                                                 | Body text (included in `update` for v0) |
| `urgency`     | `"low"` \| `"normal"` \| `"critical"`                  |                                         |
| `timeout_ms`  | i32                                                    | `-1` = persist until dismissed          |
| `icon`        | `{ "name": string }` \| `{ "path": string }` \| absent |                                         |
| `has_actions` | bool                                                   |                                         |
| `timestamp`   | i64 \| absent                                          | Unix seconds, optional                  |

### `Event`

```json
{"kind": "update", "items": [ ...MinimalNotification ]}
{"kind": "reload"}
```

| `kind`   | Payload | When |
| -------- | ------- | ---- |
| `update` | `items: MinimalNotification[]` — full active set snapshot | queue changes |
| `reload` | — | daemon config reloaded via `reload` |

---

## Pause behavior

When the daemon is **paused**:

- New `Notify` calls are **accepted** (ids are assigned) but held internally.
- `list` and `update` events show only the **pre-pause** active set.
- `unpause` moves held notifications into the active set and emits an `update`.

`dismiss` / `activate` work on both active and held notifications by id.

---

## `subscribe` stream

```
→ {"v":1,"cmd":"subscribe"}
← {"v":1,"type":"event","event":{"kind":"update","items":[]}}
   ... further `update` / `reload` event lines when state changes
```

While subscribed, the client may send additional requests on the same connection (`ping`, `list`, `dismiss`, …). A second `subscribe` on the same connection returns `INVALID_REQUEST`.

---

## Golden examples

| File                                    | Commands                         |
| --------------------------------------- | -------------------------------- |
| `examples/ipc-examples/ping.jsonl`      | `ping`                           |
| `examples/ipc-examples/list.jsonl`      | `list`                           |
| `examples/ipc-examples/subscribe.jsonl` | `subscribe` (initial event only) |
| `examples/ipc-examples/activate.jsonl`  | `activate`                       |
| `examples/ipc-examples/reload.jsonl`    | `reload`                         |
| `examples/ipc-examples/pause.jsonl`     | `pause`                          |

---

## Security

- Socket lives under `$XDG_RUNTIME_DIR` with mode `0600` on create.
- No authentication in v0 (session user only).
