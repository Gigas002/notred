# notred IPC v1 (Phase 0)

## Transport

- **Socket:** Unix domain stream, default `$XDG_RUNTIME_DIR/notred.sock` (overridable in `notred.toml`)
- **Framing:** newline-delimited JSON (NDJSON) — one JSON object per line, UTF-8
- **Version:** every line includes `"v": 1`

## Audience

- **Integrators:** use **`notredctl`** (`notredctl --help`, JSON on stdout) — see [`PLAN.md`](PLAN.md) §4.6.
- **Implementors:** this document + `examples/ipc-examples/*.jsonl` golden fixtures.

Phase 0 implements **`ping`**, **`subscribe`**, and **`list`** only. Further commands land in later phases ([`PLAN.md`](PLAN.md) §4.1).

---

## Requests

```
{"v":1,"cmd":"<command>"[, ...args]}
```

| `cmd`                           | Extra fields | Phase |
| ------------------------------- | ------------ | ----- |
| `ping`                          | —            | 0     |
| `subscribe`                     | —            | 0     |
| `list`                          | —            | 0     |
| `get`, `dismiss`, `activate`, … | TBD          | 1+    |

---

## Responses

### Success

```
{"v":1,"type":"<type>"[, ...fields]}
```

| `type`  | Fields                           | Reply to             |
| ------- | -------------------------------- | -------------------- |
| `pong`  | —                                | `ping`               |
| `items` | `"items": MinimalNotification[]` | `list`               |
| `event` | `"event": Event`                 | `subscribe` (stream) |

### Error

```
{"v":1,"error":{"code":"<CODE>","message":"..."}}
```

| `code`            | Meaning                           |
| ----------------- | --------------------------------- |
| `NOT_FOUND`       | id / resource missing (Phase 1+)  |
| `NOT_IMPLEMENTED` | command not available yet         |
| `INVALID_REQUEST` | malformed JSON or protocol misuse |

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
```

Phase 0: `subscribe` sends one initial `update` with `items: []` and keeps the connection open for further `event` lines (none until the queue exists in Phase 1).

---

## `subscribe` stream

```
→ {"v":1,"cmd":"subscribe"}
← {"v":1,"type":"event","event":{"kind":"update","items":[]}}
   ... further event lines when the active set changes (Phase 1+)
```

While subscribed, the client may send additional requests on the same connection (`ping`, `list`). A second `subscribe` on the same connection returns `INVALID_REQUEST`.

---

## Golden examples

| File                                    | Commands                         |
| --------------------------------------- | -------------------------------- |
| `examples/ipc-examples/ping.jsonl`      | `ping`                           |
| `examples/ipc-examples/list.jsonl`      | `list`                           |
| `examples/ipc-examples/subscribe.jsonl` | `subscribe` (initial event only) |

---

## Security

- Socket lives under `$XDG_RUNTIME_DIR` with mode `0600` on create.
- No authentication in v0 (session user only).
