# Remove Bot Followers on X (Twitter)

Remove bot and fake followers from your X (Twitter) account. Free, open source. Detect bots, dry-run, bulk remove followers.

**Your follower list should be yours to manage.** Remover helps you clear inactive followers while keeping verified accounts and people you follow.

See your followers, understand each decision, and choose who stays. The Chrome manager and terminal use the same local worker and saved queue. Approve one cleanup pass, then let it run in the background.

[Install and set up](#install-and-set-up) · [Run a cleanup](#bulk-remove-followers) · [Rules](#bot-detection-rules) · [FAQ](#faq) · [Download](https://github.com/altonwells/x-bot-follower-remover/releases)

![Remover visual follower manager with keep, planned removal, and queue views](docs/manager-preview.png)

*Previews use fictional accounts. The `remover --demo` command makes no X requests. You can load your real follower list and check activity without approving removal.*

## Remove fake followers

Bought followers, fake accounts, and inactive followers can leave you with a list you no longer want. Remover gives you a clear rule and control over the work.

It checks each follower and removes an account only when **all three** conditions are met:

- The latest visible post is at least **30 days** old, or the account has no posts and is at least 30 days old.
- **You do not follow** the account.
- The account is **not verified**.

These rules identify inactive followers, not proof that a person is a bot. If X returns no usable date or the request fails, the account is set aside for a retry.

Login data stays in Chrome. Progress stays on your Mac. There is no hosted service or X API subscription to set up.

## Install and set up

You need an **Apple Silicon Mac**, **Google Chrome**, and an X account. No GitHub login is required.

### 1. Install Remover

Paste this command into Terminal:

```sh
curl -fsSL https://raw.githubusercontent.com/altonwells/x-bot-follower-remover/main/install.sh | sh
```

The installer checks the download checksum. It installs in your home folder without `sudo`, Rust, or Node.

### 2. Open the setup guide

```sh
remover
```

Keep the terminal open. Remover checks for the Chrome extension and shows the next step. An old pairing does not count as a live connection.

![Remover onboarding guide with Chrome extension installation steps](docs/previews/onboarding.png)

### 3. Add the Chrome extension

1. Press **Enter** in the setup guide. It reveals the extension folder in **Finder**, opens **Chrome's extension manager**, and copies the folder path.
2. Turn on **Developer mode**. Select **Load unpacked**.
3. Press **Cmd+Shift+G**, paste the copied path, and select the folder.
4. Open **Remover**, the extension with the **R** icon. Its manager opens; select **Connection & setup** to see the automatic pairing status.

Chrome requires this manual installation step. You do not need to copy a pairing secret.

If you need the folder path:

```text
~/.local/share/remover/bundle/remover-extension
```

### 4. Connect your X account

Select **Connection & setup**, then **Open X** in the extension. Sign in using the **same Chrome profile** where you installed Remover. Return to the terminal and confirm the account shown.

![Remover Chrome extension connection page](docs/options-preview.png)

**Setup does not start removal.** The terminal shows the cleanup rule after the live account check succeeds.

If setup waits for Chrome, check that the R extension is enabled. Press **r** in the setup guide to check again. Chrome profile detection is a hint; only a live connection confirms that the extension is ready.

## See who stays and who goes

Open the **R extension** to use the visual manager. Keep `remover` open during setup and manual checks. Once you approve a cleanup, its worker runs in the background.

1. Select **Load followers** to read your follower list.
2. Select **Check activity** when collection finishes.
3. Review **Keeping**, **Planned removal**, and **Needs a check**. Open a profile to see its activity, counts, relationship, and reason. Select **Keep this account** to exclude it from pending removals.

Loading and checking do not remove anyone. **Planned removal** means an account passes the saved rules. **Removal queue** means it is in the approved queue. **Removed** shows confirmed results.

![Remover account details and Keep control](docs/manager-detail-preview.png)

## Bulk remove followers

Select **Start cleanup** in the browser manager and approve the account and rule shown. Or run `remover`, press **Enter**, then **y**.

This approves a full pass. The worker collects the list again, checks activity, and queues matches as it goes. Your Keep choices stay in effect.

The worker collects followers, checks activity, queues matches, and removes them one at a time. It saves progress as it works. You can close the terminal and run `remover` later to see progress.

![Remover stateful ladle and X animation during background cleanup](docs/previews/auto-worker-animation.png)

**Keep Chrome open, X signed in, and your Mac awake.** Work can continue over several days. It pauses while the Mac sleeps. In settings, you can turn on **Keep Mac awake** to prevent idle sleep; it does not guarantee operation with the lid closed.

| Control | Action |
| --- | --- |
| **Space** | Pause or resume the worker |
| **`,`** | Open processing settings |
| **Enter** | Show worker details |
| **v** or **Tab** | Switch between the animation and list |
| **q** | Close the view; keep the worker running |
| **c** | Cancel remaining work |
| **x** | Stop the worker and save progress |

You can also control it from another terminal:

```sh
remover status
remover pause
remover resume
remover stop
```

After a restart, run `remover` to restore a saved job. It does not start at login. A manual pause stays paused. A completed or cancelled pass needs new approval.

Removal reduces your follower count. It does not unfollow accounts you follow. There is no restore-followers action. A removed account can follow you again if your account is public.

### Advanced terminal mode

```sh
remover --advanced
```

The advanced view restores the full follower inventory, evidence panel, selection, rules, and queue controls. Before a background job starts, **Tab** switches between simple and advanced mode. Press **?** for controls.

During a background job, `--advanced` opens the work list first. Press **v** to show the continuous ladle and X animation. Account tags appear only after a removal is confirmed. Pausing stops the water; cooldowns show their remaining wait. Use `--no-animation` for a still view.

Full Auto always uses the 30-day rule and keeps verified accounts, people you follow, and Keep exceptions. Custom advanced rules apply to manual selection.

### Processing speed

Select **Processing settings** in Chrome, or press **`,`** in the terminal. Use **↑/↓** to choose a field and **←/→** to change it. **Enter** saves. **Shift+R** restores the starting settings.

| Setting | Starting value |
| --- | --- |
| Time between removals | 60 seconds |
| Attempts per batch | 20 |
| Rest between batches | 5 minutes |
| Hourly attempt limit | 50 |
| Keep Mac awake | Off |

X response limits can add longer waits. Current cooldowns finish before new speed settings take effect. These settings are local controls, not X quotas or a guaranteed completion time.

## Bot detection rules

The standard pass uses the **30-day inactivity rule**, plus protection for verified accounts and people you follow. Accounts marked to keep remain protected.

The standard pass reads the top of the Posts timeline and uses the newest valid post date it returns. It does not require full history or separate replies and reposts checks. If the main timeline is unavailable or has no date, it tries the combined posts/replies endpoint once. An old pinned post alone does not qualify. A zero-post account must be at least 30 days old. A suspicious username or a low post count alone does not authorize removal.

Activity results are saved and reused at removal. The extension checks the signed-in identity, verification, and following relationship before it removes an account. A post made after the saved activity check may remain undetected.

Unreadable results are retried after 1 minute, 15 minutes, and 6 hours. After four failures, the account is set aside for a later pass. Other accounts continue. If a removal result is uncertain, the worker checks the follower relationship before another attempt.

## Update Remover

Stop the worker before updating:

```sh
remover stop
```

Run the install command again. Reload the R extension at `chrome://extensions`, then run `remover`.

**Moving from `forgive-me`?** Stop the old worker with `forgive-me stop`. Load the new extension folder shown above and disable the old extension. Remover repairs the old default pairing automatically and keeps your saved data. Do not delete the old data folder. A custom installation may need `remover pair --reset` while the app is closed.

## Why bots follow you

Spam operators may follow accounts to attract attention, make accounts look active, or inflate follower counts. X describes fake engagement and coordinated account abuse in its [authenticity policy](https://help.x.com/en/rules-and-policies/authenticity).

A quiet timeline does not prove that an account is fake. Remover helps you apply a clear follower cleanup rule instead of guessing from a username.

## FAQ

**Does X notify the removed follower?**

Remover sends no message. X documents [removing a follower](https://help.x.com/en/using-x/following-faqs), but that page does not promise a notification policy for removal. The person can still notice the change or follow you again.

**Can I bulk remove followers?**

Yes. Approve one pass. Remover queues matching followers and removes them individually. You can pause, resume, or stop the queue.

**Is it safe? Will I hit rate limits?**

Rate limits and account restrictions are possible. There is no guaranteed safe daily allowance. Remover spaces requests, rests between batches, and waits when X reports limits. Those controls do not override [X's automation rules](https://help.x.com/en/rules-and-policies/x-automation).

**Does it work on mobile?**

No. This release needs an Apple Silicon Mac and desktop Google Chrome.

**Can I try it without removing followers?**

Yes. Select **Load followers**, then **Check activity** in the browser manager. Review the results without selecting Start cleanup. You can also run `remover --demo` to explore with fictional accounts and no X requests.

**Can it run while the terminal is closed?**

Yes. The worker continues after you close the terminal. Keep Chrome signed in and the Mac awake. Run `remover` again to see progress.

## Compared to other tools

These are documented approaches, not a live reliability test. X compatibility can change.

| Tool | Approach | Main difference |
| --- | --- | --- |
| [xbotremover](https://github.com/vanrohan/xbotremover) | Browser extension with adjustable rules | Documents live dry-run support and Chrome/Firefox builds |
| [x-bot-sweeper](https://github.com/sleeyax/x-bot-sweeper) | Semi-automatic identification and blocking | Archived; maintainer cites changing X endpoints |
| [x-bot-cleaner](https://github.com/iuzn/x-bot-cleaner) | Mark accounts Real/Bot, then bulk remove | Classification happens in the browser |
| x-follower-cleaner, the earlier local [X-Cleaner](https://github.com/taqui-786/X-Cleaner---Followers-Following) fork | Browser-side follower and following cleanup | Earlier extension approach before this terminal and worker |
| **[Remover](https://github.com/altonwells/x-bot-follower-remover)** | **Terminal + Chrome extension + saved local queue** | **Visual manager, advanced TUI, one 30-day auto rule, saved progress, and retries** |

## Free and open source

Remover is public under the **[MIT license](LICENSE)**.

[Build guide](docs/DEVELOPMENT.md) · [Operation guide](docs/GUIDE.md) · [Verification record](docs/VERIFICATION.md) · [Protocol](protocol/README.md)

Automated tests use synthetic X responses and isolated workers. They do not establish live reliability over multiple days.
