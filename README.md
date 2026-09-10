# forgive-me

A small, local X follower cleaner. **Ratatui controls it. Chrome does the X work.**

Scan your followers, review accounts matching your cleanup policy, keep exceptions, and remove the selected followers. There is no hosted service, subscription, AI model, or external database.

**Status:** runnable implementation with a fake-account demo and automated tests. The private X adapter has not been qualified against a signed-in live account. Discovery of an operation is not proof that X accepts it. No live followers were removed during development.

## Install from your terminal (Apple Silicon macOS)

Requires the [GitHub CLI](https://cli.github.com/) authenticated to an account with access to this private repository. If needed, install it with `brew install gh`, then run `gh auth login` once.

```sh
gh api repos/altonwells/forgive-me/contents/install.sh -H 'Accept: application/vnd.github.raw+json' | sh
```

The installer downloads the latest private release, verifies its SHA-256 checksum, and installs without `sudo`, Rust, Node, or Python:

- Executable: `~/.local/bin/forgive-me`
- Stable Chrome extension directory: `~/.local/share/forgive-me/bundle/forgive-me-extension`
- App files and licenses: `~/.local/share/forgive-me/bundle/`

It adds `~/.local/bin` to your zsh/bash startup file once. Open a new terminal afterward, or run `export PATH="$HOME/.local/bin:$PATH"` in the current one. It never copies or changes X credentials. Chrome's unpacked-extension installation remains a manual step.

```sh
forgive-me --demo
```

The demo uses six fictional accounts and an in-memory database. Try `a`, `d`, then Enter to cancel; `d`, then `y` runs fake removals. During a batch, a ladle pours water onto an X beside your handle, the current target, and **“Reseting followers”**—a counter of verified removals. The illustration pauses when work stops; small terminals get a compact version. Use `--no-animation` for a still illustration.

To use your real account:

1. Run `forgive-me`. First launch opens a guided setup screen.
2. Follow **Install** to load the extension in Chrome. Press `y` to copy its folder path; use Cmd+Shift+G in Chrome's folder chooser to paste it. If already loaded, press Enter.
3. In **Pair**, open the extension from Chrome's puzzle icon, enter the displayed port, press `y` to copy the secret, paste it into the extension, and click **Save & connect**. The secret stays hidden unless you press `v`.
4. Keep X open and signed in. Setup detects Chrome and checks the account automatically. If identification fails, refresh X and extension discovery, then press `r` to retry.
5. Confirm the displayed account, press Enter to open followers, then `s` to scan. It collects following, then followers, then assesses candidates. Future launches open the follower list; `Shift+P` reopens pairing and pauses work.
6. Review evidence with Enter; `K` keeps an account. `m` shows matching accounts, `a` selects matches, and `d` reviews a capped batch. Enter cancels; only `y` approves removal.

Keep Chrome and the terminal open during work. Reconnects start the controller paused. If an X operation is unavailable, refresh the relevant X page, choose **Refresh X discovery**, then reconnect. Incompatible responses stop work or leave evidence unknown; they are not treated as proof of inactivity.

## Update, select a version, or uninstall

Quit the TUI, rerun the install command, then click **Reload** on the extension in `chrome://extensions`. The stable extension path preserves its unpacked identity. Your SQLite database and pairing settings stay in the separate application-data directory.

```sh
# Pin an available release instead of installing latest:
gh api repos/altonwells/forgive-me/contents/install.sh -H 'Accept: application/vnd.github.raw+json' | sh -s -- --version v0.1.0

# Uninstall binaries and extension files; keep cleanup data and pairing settings:
sh ~/.local/share/forgive-me/bundle/install.sh --uninstall
```

Remove the extension from Chrome manually after uninstall. The shared `~/.local/bin` PATH entry is retained because other tools may use it. Shell configuration can be left unchanged with `--no-modify-path`. Custom dedicated locations use `FORGIVE_ME_INSTALL_DIR` and `FORGIVE_ME_BIN_DIR`; add a custom binary directory to PATH yourself, and use the same overrides when uninstalling.

From a local checkout, `sh install.sh --from ./dist` installs the checksummed artifacts without GitHub access. The ZIP bundle can also be unpacked and its `forgive-me` executable launched directly. This release provides Apple Silicon macOS binaries; other platforms currently require building the source.

## Default policy

An account qualifies only when fresh evidence confirms **all** of these:

- It follows you.
- You do not follow it.
- It is unverified, including blue verification.
- Its visible posting activity predates 90 days, or a fresh profile reports zero current posts.
- It is public and is not on your keep list.

Unknown verification, relationships, visibility, or activity exclude an account. Protected accounts are skipped. Zero posts does not mean the account never posted. Inactivity describes visible posting, including replies and repost actions, not whether someone reads X. These filters identify cleanup candidates, not proven bots.

`f` adjusts the threshold, verified/following exclusions, zero-post inclusion, delay and batch cap. Defaults: 10 seconds between completed writes and subsequent dispatches, at most 50 targets per approval. No pace guarantees immunity from X restrictions.

## Keys

| Key | Action |
|---|---|
| Arrows / `j`, `k`, PgUp/PgDn | Navigate |
| `s` | Start/resume scan |
| `/` | Search loaded followers |
| `m` | Toggle matching-only view |
| Enter | Evidence details; saves filters; cancels removal confirmation |
| Space / `a` | Toggle account / select currently visible matches |
| `K` | Keep/unkeep focused account |
| `f` | Edit policy |
| `d`, then `y` | Review, then approve exact removal batch |
| `p` | Pause/resume in cleanup screens |
| `c` | Cancel remaining batch in cleanup screens |
| `r` | Reconcile an uncertain outcome without replaying removal |
| `Shift+P` | Open the pairing guide and pause work |
| `o` | Open profile in Chrome |
| `?` / Esc | Help / close dialog |
| `q` / Ctrl-C | Quit; Ctrl-C works in every screen |

Setup stays paused; cleanup shortcuts are inactive there. `p` and `c` remain reserved stop controls in cleanup screens, even inside search. Pausing cannot recall a request already dispatched. Keeping a target during its preflight stops further dispatch; an already sent removal may still complete.

## How it works

The single Rust executable hosts the TUI, controller, embedded SQLite and a WebSocket listener bound to `127.0.0.1`. The MV3 extension connects with a random pairing secret; the first authenticated connection pins its extension ID. A versioned, bounded protocol carries normalized account facts and named commands. X cookies and authorization headers stay in Chrome.

The extension discovers current GraphQL operations from observed X requests and loaded X JavaScript, using the existing browser session. It fetches graph pages and candidate activity, then rechecks identity and eligibility immediately before each native `RemoveFollower` request. It never substitutes unfollowing or block/unblock. Removal shrinks your incoming follower list; public accounts can be followed again.

There is one outstanding task. The controller saves every attempted removal before dispatch; the extension journals its own dispatch/result receipt. Results are acknowledged after database persistence. Lost responses become **uncertain** and require a read-only relationship reconciliation. Neither restart nor a lost acknowledgement blindly repeats a write. SQLite records and progress are partitioned by owner ID.

Only independently verified removals increment the removed counter. `already_absent` is reported separately. Completed targets cannot be replayed because an older batch checkpoint survived a crash.

## Data and diagnostics

Default macOS data directory: `~/Library/Application Support/forgive-me/`. It contains `cleanup.sqlite`, private `config.json`, and a process lock. Database content is local, not encrypted. Back up the directory with the application closed.

```sh
forgive-me doctor
forgive-me --port 47832 pair
forgive-me --data-dir /path/to/private-folder
forgive-me pair --reset
```

`doctor` prints local configuration, saved owner, pairing identity and unresolved count; it does not contact X or certify adapter compatibility. Stop the TUI before running pairing/doctor for the same data directory. Changing the extension install directory can change its unpacked ID; use `pair --reset` if needed. Pairing settings are private: do not share them or screenshots containing them.

## Build and test

Requires Rust 1.88+ and Node 22+ for development. Rust dependencies and npm dependencies are locked. Ratatui is pinned to 0.30.2.

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cd extension
npm ci
npm run check
npm test
cd ..
./scripts/package.sh
python3 tests/install_test.py
```

The packaging script produces the native executable for the current machine, an unpacked extension and its ZIP. It does not install the extension or change your X account. See [verification](docs/VERIFICATION.md) for tested behavior and remaining live checks, and [protocol](protocol/README.md) for the browser boundary.

## Limits

X's undocumented web interfaces and automation restrictions can change. Rate limits, authentication failures, incomplete data and unknown outcomes pause work or exclude accounts. First use should be a small reviewed trial after read-only collection works. This build has no unfollow, hard-block, background scheduler, continuous monitoring, or multi-account management UI; following data is used to protect mutual relationships.

MIT licensed. See [third-party references](THIRD_PARTY_NOTICES.md).
