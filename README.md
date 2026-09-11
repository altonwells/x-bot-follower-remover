# forgive-me

Control who follows you on X.

## Why

Bought followers and unwanted accounts can leave you with a follower list you did not choose.
forgive-me helps you examine that list and remove unwanted followers.
You set the rules. You approve each removal batch.

## How

The app separates the work into four steps:

1. **Collect.** The Chrome extension reads your followers and the accounts you follow.
2. **Check activity.** You start the activity check. Missing evidence stays unknown.
3. **Review.** The terminal marks removal candidates and explains which accounts are protected.
4. **Remove.** You approve the queue. The extension checks each target again and removes one follower at a time. The controller can run in the background.

X login data stays in Chrome. The app stores account data and progress on your computer.
Unknown identity, verification, or relationship data excludes the account. Sparse accounts with old observed posts can qualify with incomplete timeline coverage.
The app does not repeat a removal automatically after an uncertain result.

## What you can do

- Find inactive followers, empty accounts, and sparse accounts with old observed posts.
- Exclude verified accounts and accounts you follow.
- Keep specific accounts, even when they meet your removal rules.
- Search the collected followers and examine account details.
- Approve removal batches, pause work, or cancel the remaining removals.
- See confirmed removals and results that need further examination.

The app has a fullscreen terminal interface and a Chrome extension.
It removes followers through X's `RemoveFollower` action.
The accounts you follow do not change.

![Follower list and account details. All accounts in this image are fictional.](docs/previews/dashboard.png)

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
9. Press Enter to open the follower list.

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

## Remove unwanted followers

Keep Chrome open and your Mac awake during work.

1. Press `s` to collect the accounts you follow and your followers. Collection stops before activity checks.
2. Press `f` to choose removal rules, then Enter to save. **REMOVE** rules identify candidates. **PROTECT** rules exclude accounts from removal.
3. Press `i` to check activity from the top in handle order. The list shows the current check and follows each account. This checks all collected followers and clears the view filter. REVIEW means incomplete evidence; KEEP means a protection or recent activity.
4. Press `m` to switch between all followers and removal candidates. This changes only the view.
5. Press Enter for evidence, or `K` to keep an account.
6. Press `a` to select checked candidates in the current view. Use **Shift+A** to select by basic rules before activity checks finish, or Space to select one.
7. Press `d` to review the full queue, then `y` to approve. New queues run in handle order. Only accounts cleared by the activity step enter the queue. Removal uses those saved results and does not scan activity again. Identity and relationship protections are still checked.
8. Press `b` to move the approved queue to the background. The current task finishes before handoff.

Basic selection applies the verification and following rules plus the keep list and account protections. It does not require completed activity evidence. Selected accounts that still need activity clearance show **CHECK FIRST** and cannot enter the removal queue. It replaces the selection with matches in the current view.

**Enter, `n`, or Esc cancels the removal confirmation.**

### Default rules

An account must meet all these conditions:

- It follows you.
- You do not follow it.
- It is unverified, including blue verification.
- It is public.
- It is not on your keep list.
- The activity step found complete inactivity for 90 days, zero current posts, or at most five posts with the newest observed activity at least 90 days old.

The sparse + old rule can qualify an account with incomplete timeline coverage. It does not claim confirmed inactivity. Use `f` to adjust the post limit (0 disables this rule). Existing approved queues retain their original rules.

The evidence panel shows the post-count percentile and log-scale z-score among collected followers with known counts (at least 30). A z-score of −2 or lower is marked LOW OUTLIER. These are review signals, not bot probabilities or independent removal rules. Numeric handles are only a weak signal.

Activity results expire after 24 hours. If a queue reaches expired or missing evidence, it pauses for a new activity review; it never performs a hidden activity rescan. Activity that changes after the saved check may not be detected before removal.

Post activity includes posts, replies, and reposts. Inactivity does not prove that an account is a bot.
Zero current posts does not mean the account never posted.

New installations use a minimum interval of 60 seconds and an hourly budget of 50 removal attempts. Existing saved settings are preserved. The full selected queue is approved at once; the hourly budget limits execution, not selection. Attempts that stop at identity or relationship checks count toward that local budget too.

X reset times and Retry-After headers can extend the wait. Cooldowns and attempt budgets survive process restarts. A transient read or rate limit before dispatch keeps the target queued. Authentication failures and uncertain writes stop work. The app does not promise a fixed completion time or immunity from X restrictions.

### Background work

After approval, press `b`. You can close the terminal after the handoff message. Run `forgive-me` again to open the background queue monitor. Closing that monitor leaves the queue running.

```sh
forgive-me status   # Current progress and wait time
forgive-me pause    # Pause the background queue
forgive-me resume   # Resume the same approved account and queue
forgive-me stop     # Stop the worker; preserve the remaining queue
```

The monitor also has pause, resume, cancel, and stop controls. To return to the full follower view, stop the worker and reopen forgive-me. Resolve uncertain actions with `r` before resuming. After a process crash or computer restart, open forgive-me and review/resume saved work; automatic launch at login is not configured.

Chrome must stay open and signed in to the approved account. The Mac must stay awake. No work runs while the computer is asleep. A normal Chrome reconnect can resume the same approved background queue; a manual pause, account change, or uncertain result prevents automatic continuation.

## Controls

| Key | Action |
| --- | --- |
| ↑ / ↓, `j` / `k`, mouse wheel | Move through the list or scroll help and details |
| `s` | Collect or continue collecting followers |
| `i` | Check collected followers from the top |
| `/` | Search collected followers |
| `m` | Switch between all followers and removal candidates |
| Enter | Open account details |
| Space | Select or deselect one account by basic rules |
| `a` | Select checked removal candidates in the current view |
| **Shift+A** | Select basic matches in this view; check activity before queueing |
| `v` | Switch the running queue between list and animation |
| `K` | Add or remove a keep exception |
| `f` | Change the removal rules |
| `d` | Review the full removal queue |
| `b` | Move an approved queue to the background |
| `p` | Pause or continue work |
| `c` | Cancel the remaining removals |
| `r` | Resolve one uncertain result without another removal request |
| `o` | Open the account's X profile |
| **Shift+P** | Open the connection guide and pause work |
| `?` | Open help |
| `q` | Quit from the follower list or connection guide |
| **Ctrl-C** | Pause and quit from any screen |

The terminal keeps the header and controls in place. Lists scroll inside the app.
The minimum window size is 52 columns by 12 rows.

To try the interface with fictional accounts:

```sh
forgive-me --demo
```

The removal queue shows the list and highlights the current account. Press `v` to show the pouring animation and confirmed removal count.
To disable the animation:

```sh
forgive-me --no-animation
```

## Update

1. Quit forgive-me.
2. Run the install command again.
3. Open `chrome://extensions`.
4. Select **Reload** on forgive-me. Accept the native messaging permission if Chrome asks.
5. Refresh your X tab.
6. Start forgive-me in the terminal.
7. Open the extension and select **Connect automatically**.

The update preserves your pairing settings, keep list, and action history.

## Limits and current status

X can change its web interface or restrict requests. The app can stop when this occurs.
A pause cannot stop a removal request that the extension has already sent.
There is no action to restore removed followers. A removed account can follow a public account again.

Version 0.1.10 passed automated, terminal, and installer tests.
The user confirmed the preceding X connection fix after loading the updated extension. The new background workflow has automated coverage; it has not run a live overnight removal test.
No real follower removal was used to test this release.
See the [test record](docs/VERIFICATION.md).

## Reference

- [Troubleshooting, data, and installation options](docs/GUIDE.md)
- [Build instructions and internal design](docs/DEVELOPMENT.md)
- [Browser protocol](protocol/README.md)
- [Release history](https://github.com/altonwells/forgive-me/releases)

[MIT license](LICENSE). [Third-party notices](THIRD_PARTY_NOTICES.md).
