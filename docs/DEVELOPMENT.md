# Development

[Return to the README](../README.md).

## Internal design

One Rust executable contains the terminal interface, work controller, and SQLite database.
Ratatui renders the interface.
A WebSocket server listens on `127.0.0.1`.

The Chrome Manifest V3 extension connects with a random pairing secret.
The first authenticated connection fixes the allowed extension identity.
X cookies and authorization headers stay in Chrome.

On macOS, TUI and worker startup register the native messaging host `com.x_bot_follower_remover.pairing` for the current user.
A small shell wrapper starts the same executable in its hidden `native-host` mode.
It passes the active data folder as an absolute path.
Chrome exchanges one length-prefixed JSON request and response over standard input and output.
The host returns the existing port and secret only while the corresponding TUI or worker holds its process lock.
It accepts only the bundled extension's origin and rejects a conflicting saved extension identity.
The secret does not enter a URL, HTTP endpoint, log, or clipboard during automatic pairing.

The extension ID is derived from its canonical installation path with Chromium's SHA-256 scheme.
The extension keeps its existing ID and stored data when the installer replaces the bundle at that path.
Manual settings remain available for repair.
Automatic pairing does not confirm the X account or start work.

The [browser protocol](../protocol/README.md) defines the allowed commands and message limits.

The extension finds X operations in observed requests and X JavaScript assets.
It reads operation definitions without execution of downloaded code.
Discovery supports responsive-web/webpack assets, x-web/Vite assets, and Relay definitions.
The extension creates a new `x-client-transaction-id` for each request.

A read request that returns HTTP 404 can refresh operation data and retry once.
Removal requests do not retry automatically.
The extension reads follower pages and account activity through the signed-in browser session.
It examines Posts, Replies, and Reposts separately.
It uses current `relationship_counts` and `tweet_counts` fields, with support for older fields.

The extension confirms the owner and removal rules again before each `RemoveFollower` request.
A recent post action excludes an account from inactivity matches.
Missing or ambiguous activity data stays unknown.

The controller permits one work request at a time. The same App controller can run without a terminal after an explicit queue handoff. A private Unix socket serves status and fixed pause/resume/cancel/stop commands. Reopening the CLI monitors that worker instead of creating a second X connection.
It records each removal attempt before dispatch.
The extension also records request and result receipts.
The controller acknowledges results only after it stores them.
A lost result requires a relationship read to resolve the outcome.
A restart does not automatically repeat a removal.

The database separates data by account ID.
Saved progress cannot cause a completed removal to run again.

## Build and test

Requirements:

- Rust 1.88 or later.
- Node 22 or later, with npm.
- Python 3 for package and installer scripts.

Cargo.lock and package-lock.json fix dependency versions.
Ratatui is pinned to version 0.30.2.

From the repository folder, run the Rust tests:

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

Run the extension tests:

```sh
cd extension
npm ci
npm run check
npm test
cd ..
```

On Apple Silicon macOS, build the release package:

```sh
./scripts/package.sh
```

Run the installer and terminal tests:

```sh
python3 tests/install_test.py
python3 tests/terminal_test.py target/release/remover
```

The package contains the executable, extension, documentation, licenses, and checksums.
The script does not install the extension or change an X account.
See the [test record](VERIFICATION.md) and [release procedure](RELEASING.md).

## Render interface previews

The preview program uses fictional accounts and Ratatui's test renderer.
It does not connect to Chrome or X.
The image renderer uses macOS AppKit without a visible window.

From the repository folder, run:

```sh
cargo run --example preview > /tmp/remover-preview.json
swift scripts/render-preview.swift /tmp/remover-preview.json /tmp/remover-previews
```

## Current scope

The app removes followers. It uses the accounts you follow to protect those relationships.
It does not include unfollow, hard-block, automatic launch at login, continuous account monitoring, or a multiple-account interface.

## Licenses

The application uses the MIT license. The signing implementation in `extension/src/transaction.ts` includes a numerical protocol port from twscrape, which credits XClientTransaction. Their copyright and MIT license notices remain in [the extension license file](../extension/THIRD_PARTY_LICENSES.txt) and each extension build.

Release bundles include `DEPENDENCY_LICENSES.md` and a `licenses/` folder with Cargo dependency license files. Development package versions and their license terms are recorded in the lock files. Reference clones are not included in release bundles.

## Visual manager and advanced terminal

`src/manager.rs` projects the controller's saved accounts, action receipts, rules, and queue into browser snapshots. `extension/src/manager.ts` renders the options page and sends owner-bound commands through `background.ts`. No frontend framework or new runtime dependency is used.

`src/ritual.rs` shares the ladle/X scene between foreground and background terminal views. Worker status includes a bounded window of confirmed receipt IDs so polling does not replay old tags. Animation uses elapsed time; the work scheduler still controls requests independently.

Generate terminal fixtures with `cargo run --example preview` and `scripts/render-preview.swift`. For the browser preview, run `python3 scripts/preview-manager.py` after building the extension. It serves fictional profiles on localhost and cannot access X.
