# Changelog

All notable changes to this project are documented in this file.

## Unreleased

### Changed

- libnotred: `Notification` and `ActivateEvent` gain a public `resident` field, so struct literals of them must set it.
- libnotred: `Queue::set_paused` returns the ids it released; `Queue::is_held` and `HostState::set_paused` are new.
- libnotred: new `HistoryStore::max_id` and `Queue::allocate_ids_after`.
- libnotred: `ActivateEvent` gains a public `summary` field.

### Fixed

- **notred exits cleanly on SIGTERM and Ctrl+C**, removing its socket, instead of being killed and leaving the socket behind.
- **Clicking a notification that has actions but no `default` action dismisses it.** Chat apps often offer only named actions such as "reply"; a click (`activate` without a key, or a `button_left`/`touch` input) failed with `unknown action key "default"` and the notification stayed. An explicit unknown key is still an error.
- **A notification closes after one of its actions is invoked**, as the Desktop Notifications spec expects, unless it sets the `resident` hint. It used to stay on screen after a click ran its action.
- **Notifications received while paused no longer expire before you see them.** Their timeout ran while they were held, so after `unpause` anything with a timeout was already gone. The timeout now starts when unpausing shows them.
- **Unpausing respects `[queue].max_visible`.** Everything held while paused became visible at once, beyond the cap.
- **notred-tui shows the history when a notification carries an image.** Its list parser knew icon names and paths but not raw `image-data` icons (chat app avatars), so one such notification made the whole history fail to load.
- **History kept across restarts (`flush = false`) is no longer overwritten.** Notification ids restart at 1 each run, and history rows are keyed by id, so new notifications replaced old entries one by one. New ids now continue after the highest id in the history.
- **Config mistakes are reported.** Unknown keys (a typo such as `default_timeout`) were ignored without a word, and a missing or invalid override fragment was logged before logging was set up, so it never appeared, while every fragment was dropped. Both now show in the log. `notredctl reload` with a broken fragment fails and keeps the running config instead of reporting success.
- A malformed IPC request now gets an `INVALID_REQUEST` reply and the connection stays open; outside `subscribe` notred hung up without an answer.
- `notred-tui --ctl` overrides `ctl` in `tui.toml`; the config file used to win over the command line.
- `notred-tui --config <file>` with a file that does not exist is an error instead of silently using defaults.
- The `on_action` hook gets `NOTRED_SUMMARY` like the other hooks; the example config lists it, but it was missing.

### Security

- The history database (`history.db`) is now readable by its owner only (mode 0600); it was created with the default umask, usually 0644, and holds notification text.
- `image-data` icons are checked before they are passed on: 8 bits per sample, 3 channels (4 with alpha), a `rowstride` of at least one row, and enough bytes for every row. Anything else is ignored and the next icon source is used, so subscribers indexing the pixels by `rowstride` (as `docs/IPC.md` tells them to) cannot read past the end.

## [0.1.0] - 2026-09-04
