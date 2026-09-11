# forgive-me

Remove inactive followers from your X account.

## Why

You want a follower list you chose. One clear rule should be enough to clean it.

## How

Remove a follower when all three conditions are true:

- They have not posted in 30 days.
- You do not follow them.
- They are not verified.

Posts, replies, and reposts count as activity. Likes do not count.
Keep exceptions remain protected. A zero-post account must be at least 30 days old.
An unreadable result goes to a retry list; it is not treated as inactivity.
Post counts, usernames, and statistical scores do not authorize removal.

Press **Enter**, review the rule, then **y** to start. The app collects the follower list, checks accounts in order, and queues matches automatically.
The worker runs in the background from the start. Close the terminal and open `forgive-me` later to see progress.

## What runs where

Chrome reads X and removes followers through `RemoveFollower`. Login data stays in Chrome.
The local Rust worker stores progress and receipts in SQLite. The TUI controls the worker.
The app does not unfollow accounts you follow.

## Install

Requirements:

- An Apple Silicon Mac.
- Chrome with access to your X account.
- The [GitHub CLI](https://cli.github.com/), signed in to an account with access to this private repository.

If you use Homebrew, install the GitHub CLI:

```sh
brew install gh
```

Sign in to GitHub:

```sh
gh auth login
```

Install forgive-me:

```sh
gh api repos/altonwells/forgive-me/contents/install.sh -H 'Accept: application/vnd.github.raw+json' | sh
```

The installer verifies the download checksum. It does not need `sudo`, Rust, Node, or Python.

Open a new terminal window. Start the app:

```sh
forgive-me
```

### Connect Chrome

Keep the terminal open during these steps.

1. Press Enter in the setup guide. The app opens Chrome and copies the extension folder path.
2. Enable **Developer mode** on Chrome's extension page.
3. Select **Load unpacked**.
4. Press **Cmd+Shift+G** in the folder selector.
5. Paste the copied path and select the folder.
6. The extension pairs with the terminal automatically.
7. Select **Open X** in the extension and sign in to your X account.
8. Check that the terminal shows the correct account.
9. Press Enter to open the cleanup screen.

Chrome requires you to confirm the extension installation once.
You do not need to enter a port or pairing secret.
The extension folder is:

```text
~/.local/share/forgive-me/bundle/forgive-me-extension
```

If the extension is already installed, press `o` to open its setup page.
On the connection screen, Enter opens the extension and `i` opens install/reload.
The guide advances only when Chrome connects and X identifies your account.
Select **Connect automatically** if it does not connect.
**Shift+P** opens the terminal guide again.
Manual pairing remains available under **Manual connection and repair**.

## Start cleanup

1. Run `forgive-me` and confirm the correct X account is connected.
2. Press **Enter**, then **y** to approve the 30-day rule for this pass.
3. Leave Chrome open and signed in. Keep the Mac awake.

There are no separate scan, activity, selection, or batch-approval steps in this flow.
Checks happen shortly before removal. The queue uses the saved result; removal does not fetch activity timelines again.
It does check your signed-in identity, verification, and following relationship before writing.
Saved approvals do not expire halfway through the job. New posts after a saved check may not be detected.

## Controls

| Key | Action |
| --- | --- |
| Enter on the start screen | Review and start cleanup |
| Space in the worker view | Pause or resume |
| `,` | Processing settings |
| Enter in the worker view | Details |
| `q` in the worker view | Close the view; keep working |
| `c` | Cancel remaining work |
| `x` | Stop the worker and save progress |

After a pass completes or is cancelled, Enter can approve another pass.
Older approved queues retain their old rules and controls until finished or cancelled.
`forgive-me --demo` retains the fictional inventory and pouring-animation preview.

## Processing settings

Choose a field with ↑/↓ and change it with ←/→. Enter or Esc saves.
Shift+R restores the recommended starting configuration:

| Setting | Starting value |
| --- | --- |
| Removal interval | 60 seconds |
| Attempts per batch | 20 |
| Rest between batches | 5 minutes |
| Hourly attempt limit | 50 |
| Keep Mac awake | Off; enable when wanted |

Settings apply to the current job without changing its removal rule.
Active cooldowns finish first. X response limits can extend the wait.
These are local defaults, not X quotas or a completion guarantee.
Read requests in the new flow are spaced by at least one second; endpoint backoff can increase that interval.

## Background work and recovery

Progress, retry deadlines, batch rests, and removal receipts survive restarts.
A normal Chrome reconnect resumes the same approved account. A manual pause stays paused.
After restarting the Mac or worker, run `forgive-me` to restore the saved job. Automatic launch at login is not configured.
No requests run while the Mac sleeps. The optional awake setting prevents idle sleep while the job runs; it does not guarantee operation with the lid closed.

Unreadable accounts are retried after 1 minute, 15 minutes, and 6 hours. After four failures they remain set aside for a later pass.
Other accounts continue. Uncertain removals are recorded, then checked through the follower relationship before that target can be retried.
An absent follower closes the old attempt. A present follower can receive a bounded new attempt under the approved rule. Unresolved writes are never blindly repeated.
Login failures, account changes, and access denials pause the job.

```sh
forgive-me status
forgive-me pause
forgive-me resume
forgive-me stop
```

## Update

Run the install command again, reload forgive-me in `chrome://extensions`, then restart the TUI.
If an old background worker is running, first use `forgive-me stop` to save and stop it.
The new workflow requires the matching extension with `simple_cleanup:1` support.
Pairing, keep exceptions, and action history are preserved. Installation does not start cleanup.

## Status

Version 0.2.0 introduces the single-rule background workflow.
Automated checks use synthetic X responses and isolated local workers. No live removal or multi-day unattended run was used to test this release.
A removed follower can follow a public account again. There is no restore-followers action.

## Reference

- [Operation guide](docs/GUIDE.md)
- [Verification record](docs/VERIFICATION.md)
- [Development](docs/DEVELOPMENT.md)
- [Protocol](protocol/README.md)
- [Releases](https://github.com/altonwells/forgive-me/releases)

[MIT license](LICENSE). [Third-party notices](THIRD_PARTY_NOTICES.md).
