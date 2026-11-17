# Changelog

All notable changes to this project are documented in this file.

## Unreleased

### Fixed

- **notred exits cleanly on SIGTERM and Ctrl+C**, removing its socket, instead of being killed and leaving the socket behind.
- **Clicking a notification that has actions but no `default` action dismisses it.** Chat apps often offer only named actions such as "reply"; a click (`activate` without a key, or a `button_left`/`touch` input) failed with `unknown action key "default"` and the notification stayed. An explicit unknown key is still an error.

## [0.1.0] - 2026-09-04
