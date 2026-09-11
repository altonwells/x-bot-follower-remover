# Remover 0.3.0

The project is now x-bot-follower-remover. Run `remover` to open the terminal app.

Chrome has a new R icon and a redesigned connection page. The queue and 30-day removal rules are unchanged. Existing cleanup data remains accessible.

When upgrading, stop the old worker, load the new extension folder, and disable the old extension. Run `remover pair --reset` with the app closed, then reopen it to pair. See the README for full steps.

This release does not start cleanup during installation or pairing.
