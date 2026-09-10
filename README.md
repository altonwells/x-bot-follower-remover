# forgive-me

A small, local X follower cleaner. **Ratatui controls it. Chrome does the X work.**

Scan your followers, review accounts matching your cleanup policy, keep exceptions, and remove the selected followers. There is no hosted service, subscription, AI model, or external database.

**Status:** runnable implementation with a fake-account demo and automated tests. The private X adapter has not been qualified against a signed-in live account. Discovery of an operation is not proof that X accepts it. No live followers were removed during development.

## Start here (macOS)

The prepared native binary is `dist/forgive-me`; the unpacked extension is `dist/forgive-me-extension/`. Development tools are unnecessary when using these artifacts.

```sh
./dist/forgive-me --demo
```

The demo uses six fictional accounts and an in-memory database. Try `a`, `d`, then Enter to cancel; `d`, then `y` runs fake removals. During a batch, a ladle pours water onto an X beside your handle, the current target, and **“Reseting followers”**—a counter of verified removals. The illustration pauses when work stops; small terminals get a compact version. Use `--no-animation` for a still illustration.

To use your real account:

1. Run `./dist/forgive-me pair` and copy its port and pairing secret.
2. Open `chrome://extensions`, enable Developer mode, choose **Load unpacked**, and select `dist/forgive-me-extension/` (the directory containing `manifest.json`).
3. Open X in that Chrome profile and sign in. Visit your following and followers pages to expose current web request definitions.
4. Start `./dist/forgive-me`. Click the extension icon, paste the pairing settings, and choose **Save & connect**.
5. Verify the account shown in the terminal. Press `s` to scan. It collects following, then followers, then assesses candidates.
6. Review evidence with Enter; `K` keeps an account. `m` shows matching accounts, `a` selects matches, and `d` reviews a capped batch. Enter cancels; only `y` approves removal.

Keep Chrome and the terminal open during work. Reconnects start the controller paused. If an X operation is unavailable, refresh the relevant X page, choose **Refresh X discovery**, then reconnect. Incompatible responses stop work or leave evidence unknown; they are not treated as proof of inactivity.

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
| `p` | Pause/resume, available in every screen |
| `c` | Cancel remaining batch, available in every screen |
| `r` | Reconcile an uncertain outcome without replaying removal |
| `o` | Open profile in Chrome |
| `?` / Esc | Help / close dialog |
| `q` / Ctrl-C | Quit; Ctrl-C works in every screen |

`p` and `c` remain reserved stop controls even inside search. Pausing cannot recall a request already dispatched. Keeping a target during its preflight stops further dispatch; an already sent removal may still complete.

## How it works

The single Rust executable hosts the TUI, controller, embedded SQLite and a WebSocket listener bound to `127.0.0.1`. The MV3 extension connects with a random pairing secret; the first authenticated connection pins its extension ID. A versioned, bounded protocol carries normalized account facts and named commands. X cookies and authorization headers stay in Chrome.

The extension discovers current GraphQL operations from observed X requests and loaded X JavaScript, using the existing browser session. It fetches graph pages and candidate activity, then rechecks identity and eligibility immediately before each native `RemoveFollower` request. It never substitutes unfollowing or block/unblock. Removal shrinks your incoming follower list; public accounts can be followed again.

There is one outstanding task. The controller saves every attempted removal before dispatch; the extension journals its own dispatch/result receipt. Results are acknowledged after database persistence. Lost responses become **uncertain** and require a read-only relationship reconciliation. Neither restart nor a lost acknowledgement blindly repeats a write. SQLite records and progress are partitioned by owner ID.

Only independently verified removals increment the removed counter. `already_absent` is reported separately. Completed targets cannot be replayed because an older batch checkpoint survived a crash.

## Data and diagnostics

Default macOS data directory: `~/Library/Application Support/forgive-me/`. It contains `cleanup.sqlite`, private `config.json`, and a process lock. Database content is local, not encrypted. Back up the directory with the application closed.

```sh
./dist/forgive-me doctor
./dist/forgive-me --port 47832 pair
./dist/forgive-me --data-dir /path/to/private-folder
./dist/forgive-me pair --reset
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
```

The packaging script produces the native executable for the current machine, an unpacked extension and its ZIP. It does not install the extension or change your X account. See [verification](docs/VERIFICATION.md) for tested behavior and remaining live checks, and [protocol](protocol/README.md) for the browser boundary.

## Limits

X's undocumented web interfaces and automation restrictions can change. Rate limits, authentication failures, incomplete data and unknown outcomes pause work or exclude accounts. First use should be a small reviewed trial after read-only collection works. This build has no unfollow, hard-block, background scheduler, continuous monitoring, or multi-account management UI; following data is used to protect mutual relationships.

MIT licensed. See [third-party references](THIRD_PARTY_NOTICES.md).
