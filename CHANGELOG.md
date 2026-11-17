# Changelog

All notable changes to this project are documented in this file.

## Unreleased

### Changed

- libnotred: `Notification` and `ActivateEvent` gain a public `resident` field, so struct literals of them must set it.
- libnotred: `Queue::set_paused` returns the ids it released; `Queue::is_held` and `HostState::set_paused` are new.

### Fixed

- **notred exits cleanly on SIGTERM and Ctrl+C**, removing its socket, instead of being killed and leaving the socket behind.
- **Clicking a notification that has actions but no `default` action dismisses it.** Chat apps often offer only named actions such as "reply"; a click (`activate` without a key, or a `button_left`/`touch` input) failed with `unknown action key "default"` and the notification stayed. An explicit unknown key is still an error.
- **A notification closes after one of its actions is invoked**, as the Desktop Notifications spec expects, unless it sets the `resident` hint. It used to stay on screen after a click ran its action.
- **Notifications received while paused no longer expire before you see them.** Their timeout ran while they were held, so after `unpause` anything with a timeout was already gone. The timeout now starts when unpausing shows them.
- **Unpausing respects `[queue].max_visible`.** Everything held while paused became visible at once, beyond the cap.

## [0.1.0] - 2026-09-04
