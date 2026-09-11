forgive-me v0.1.6 fixes interruption of the X account check during page loads.

- X page completion sends a readiness notification instead of disconnecting and reconnecting the extension.
- The terminal lets an active identity check finish. A queued readiness event can cause one retry after a recoverable failure. It does not retry a successful check or a rate limit, access denial, or account change.
- The terminal shows the actual account-check error and lets Enter retry while Chrome stays paired. Rate-limit cooldowns stay in effect during setup navigation and reconnection.
- The readiness notification cannot start scans, removals, or other cleanup work.

Update both parts: quit and reopen forgive-me, then Reload the extension in chrome://extensions. Keep the TUI open and refresh your signed-in X tab. Older terminals do not understand the new page-ready notification.

The reconnect regression was reproduced in a test before the fix. Tests use synthetic X responses and local connections. Live X compatibility remains unverified, and no account was scanned or modified.
