# Operation guide

[Return to the README](../README.md).

## If the extension does not connect

1. Start remover in the terminal.
2. Reload the extension in `chrome://extensions` after an update.
3. Open the extension settings.
4. Select **Connect terminal**.

The extension and X must use the same Chrome profile.
The app registers its local pairing helper when the terminal starts.
The helper gives the extension the port and secret through Chrome's native messaging pipe.

If automatic pairing fails:

1. Press **Shift+P**, then `m` in the terminal to open manual pairing.
2. Open **Connection help** in the extension.
3. Enter the port shown in the terminal.
4. Press `y` in the terminal to copy the pairing secret.
5. Paste the secret into the extension and select **Save & connect**.

The pairing secret gives the extension access to the local app. Keep it private.
Manual connection settings remain in use until you select **Connect terminal**.

If Chrome is paired but the terminal cannot identify your X account:

1. Read the error in the **X account check failed** panel.
2. For a login or missing-tab error, press `b` and sign in to X in this Chrome profile.
3. For an operation or signing error, press `o` to open the extension. Expand **Connection help** and select **Refresh X discovery**.
4. Press Enter or `r` in the terminal to retry the account check.

A page load does not disconnect Chrome. If a check is running, the app lets it finish.
A new page can trigger one retry after a recoverable failure.
Rate limits require waiting. Reconnecting is not a way to resolve an X access denial.

The guide stays paused. Removal controls are not active there.
The new worker resumes after a normal reconnect only for the approved owner. Manual pause and authentication failures stop automatic continuation. Uncertain removals are recovered separately.

## Run a cleanup

Press Enter on the start screen, then y. Collection, activity checks, queueing, and removal are automatic.
The fixed rule is no posts in 30 days, not verified, and you do not follow the account.
Posts, replies, and reposts count. Zero-post accounts must be at least 30 days old.
Keep exceptions remain protected. Unavailable evidence is retried, never substituted with an old pinned post or a low post count.

The terminal opens the background monitor after approval. q closes only the monitor.
Space pauses or resumes. Enter shows details. Comma opens processing settings.
Enable Keep Mac awake there for long runs if needed. Chrome must stay open and signed in.

## When an account cannot be read

The worker stores a retry deadline and continues with other accounts.
It retries after 1 minute, 15 minutes, and 6 hours, then leaves the account aside after four failures.
The Retry later count remains visible. Start another pass after fixing an adapter issue to revisit these accounts.
A cursor loop or incomplete follower collection stops collection without claiming the list is complete.

## When a removal is uncertain

The worker stores the outcome before acknowledging it to Chrome.
It later reads the follower relationship. It does not blindly repeat the removal.
If the account is absent, the attempt is closed. If present, the job may make a bounded new attempt.
If the read remains unavailable, that account stays isolated while others continue.
Authentication failures or an account switch pause all work.

## Restart and update

Run remover again to attach to the worker. A saved manual pause remains paused.
If the process or computer restarted, opening remover restores the approved job.
There is no automatic launch at login.

For updates, stop any old worker with `remover stop`, install the release, reload the extension, and reopen remover.
An existing legacy approval keeps its rules. To use the new 30-day workflow, cancel the old job, then start a new cleanup.
The installer preserves pairing, the SQLite database, keep exceptions, and receipts.

## Data and diagnostic commands

```sh
remover status
remover pause
remover resume
remover stop
remover doctor
```

Default data directory: `~/Library/Application Support/remover`.
The database contains account evidence and removal receipts. The configuration contains the local pairing secret.
Do not share the configuration file. Back up the database before manually changing or removing data.
Use `--data-dir` only for a separate installation or isolated test; it does not share existing recovery records.

[Return to README](../README.md).
