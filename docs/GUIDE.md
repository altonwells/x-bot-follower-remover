# Operation guide

[Return to the README](../README.md).

## If the extension does not connect

1. Start forgive-me in the terminal.
2. Reload the extension in `chrome://extensions` after an update.
3. Open the extension settings.
4. Select **Connect automatically**.

The extension and X must use the same Chrome profile.
The app registers its local pairing helper when the terminal starts.
The helper gives the extension the port and secret through Chrome's native messaging pipe.

If automatic pairing fails:

1. Press **Shift+P**, then `m` in the terminal to open manual pairing.
2. Open **Manual connection and repair** in the extension.
3. Enter the port shown in the terminal.
4. Press `y` in the terminal to copy the pairing secret.
5. Paste the secret into the extension and select **Save & connect**.

The pairing secret gives the extension access to the local app. Keep it private.
Manual connection settings remain in use until you select **Connect automatically**.

If the terminal cannot identify your X account:

1. Refresh the signed-in X tab.
2. Select **Refresh X discovery** in the extension settings.
3. Press `r` in the terminal's connection guide.

The guide stays paused. Removal controls are not active there.
A connection or reconnection does not automatically continue work.

## If a scan stops

The scan first collects accounts you follow, then followers, then account activity.
The scan status shows the current step.
Mutual accounts can appear before the follower scan finishes.
The displayed list can be incomplete until collection finishes.

If the terminal reports an unavailable X operation:

1. Reload the extension in `chrome://extensions`.
2. Refresh the signed-in X tab.
3. Select **Refresh X discovery** in the extension settings.
4. Select **Connect automatically**.
5. Press `s` in the terminal to start or continue the scan.

Scans from before version 0.1.2 require new account data.
The app collects this data again when you press `s`.
Your keep list and action history stay in place.

An unknown result is not evidence of inactivity.
If required data stays unknown, the default rules exclude that account.

## Pause, cancel, and uncertain results

In cleanup screens, `p` pauses or continues work. `c` cancels the remaining removals.
These keys also control work when the search field is open.
They do not enter search text.

A removal request can finish after you press pause or cancel.
The extension cannot recall a request that it has already sent.

An **uncertain** result means the app cannot confirm whether a removal occurred.
Press `r` to examine the relationship again.
This action does not send another removal request.
The app does not continue removals while an uncertain result remains.

The removal counter includes only confirmed removals.
An account that was already absent does not increase the counter.

## Terminal behavior

The app uses a separate fullscreen terminal display.
It restores the shell display when you quit normally.
The mouse wheel moves the selection or scrolls the open help or account details.

To select terminal text, use your terminal's selection modifier.
This is often Shift or Option.

`q` closes help and account details. It quits from the follower list or connection guide.
**Ctrl-C** pauses and quits from any screen.

## Installed files

| Item | Default location |
| --- | --- |
| Command | `~/.local/bin/forgive-me` |
| App files and licenses | `~/.local/share/forgive-me/bundle/` |
| Chrome extension | `~/.local/share/forgive-me/bundle/forgive-me-extension/` |
| Chrome pairing host | `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/com.forgive_me.pairing.json` |
| Account data and settings | `~/Library/Application Support/forgive-me/` |

The data folder contains `cleanup.sqlite`, `config.json`, and a process lock.
The database stores account data, keep exceptions, scan progress, and action history.
Database contents are not encrypted.

Before you copy the data folder as a backup, quit the app.
Do not share `config.json` or an image that shows the pairing secret.

## If the command is not found

The installer adds `~/.local/bin` to your zsh or bash startup file once.
Open a new terminal window after installation.

To use the current terminal window, set its command path:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

## Installation options

To install a specific release:

```sh
gh api repos/altonwells/forgive-me/contents/install.sh -H 'Accept: application/vnd.github.raw+json' | sh -s -- --version v0.1.4
```

To install a local package from the repository folder:

```sh
sh install.sh --from ./dist
```

Local packages include checksums. This installation method does not need GitHub access.
You can also extract the release ZIP and start its `forgive-me` executable.
The supplied executable requires Apple Silicon macOS.
Other platforms require a source build.

Use `--no-modify-path` to leave shell startup files unchanged.
Custom installation folders use these environment variables:

- `FORGIVE_ME_INSTALL_DIR`: a folder for forgive-me app files.
- `FORGIVE_ME_BIN_DIR`: a folder for the command.

Use a dedicated app folder. Add the custom command folder to your shell's command path.
Use the same variables when you uninstall.

## Uninstall

1. Quit forgive-me.
2. Run this command:

```sh
sh ~/.local/share/forgive-me/bundle/install.sh --uninstall
```

3. Remove the extension from `chrome://extensions`.

The uninstaller removes the Chrome pairing host if it belongs to this installation.
It preserves account data and pairing settings.
It also preserves the shared `~/.local/bin` entry in your shell startup file.

## Diagnostics

Quit the app before you use these commands with the same data folder.

To show local settings and unresolved action counts:

```sh
forgive-me doctor
```

This command does not contact X. It cannot confirm that the X integration works.

To use a different local port:

```sh
forgive-me --port 47832 pair
```

To use a different data folder:

```sh
forgive-me --data-dir /path/to/private-folder
```

If you must replace the pairing secret or extension identity:

```sh
forgive-me pair --reset
```

This command replaces the old pairing secret. Pair the extension again after the reset.
A change to the extension installation folder can change its Chrome identity.
