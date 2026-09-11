forgive-me v0.1.5 replaces the page-by-page pairing guide with setup driven by the live connection.

- One primary Enter action opens the next required screen.
- Terminal, Chrome, and X account status show what is ready and what is missing.
- Setup advances only after Chrome connects and X identifies the account. Arrow keys cannot skip these checks.
- Disconnect returns to pairing. Press i there to install or reload; no backward navigation is needed.
- The layout is centered and compact, with a fixed action bar. Small terminals show a scroll hint and keep actions visible.
- Manual repair stays available. Opening setup pauses work; confirming the account does not start a scan or removals.

Quit and reopen forgive-me to use the new wizard. Press Shift+P if the follower list is already open. The Chrome pairing mechanism is unchanged from v0.1.4.

Verification uses synthetic connection events, rendered previews, an actual pseudo-terminal, native messaging integration, and installer tests. No live X account was scanned or modified.
