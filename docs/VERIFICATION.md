# Verification record

Implementation date: September 10, 2026. Native target: Apple Silicon macOS (`aarch64-apple-darwin`), built with Rust 1.94.1.

Latest result (v0.1.2): **23 Rust tests and 41 extension tests passed**. Clippy, Rust formatting, TypeScript checking, release build and extension bundling passed. Both ZIP archives passed integrity checks.

## Automated checks

- Rust unit and integration tests cover strict policy/unknown handling, terminal sanitization, keep-list persistence, account isolation, duplicate dispatch rejection, restart recovery, actual loopback WebSocket pairing and origin/session rejection, default-cancel confirmation, reconnect invalidation, modal stop controls, completed-action checkpoint recovery, shared Rust/TypeScript policy fixtures, and empty/narrow terminal rendering.
- Extension tests cover response parsing, verification flags, exact ID strings, pagination, relationship absence, empty/tombstoned activity, pinned posts, repost timestamps, operation discovery, allowlisted commands, duplicate work, uncertain journaling, acknowledgement/reconciliation, asynchronous mutual exclusion, disconnect/pause generation invalidation, and preflight versus post-dispatch errors.
- `cargo clippy --all-targets -- -D warnings`, Rust formatting, TypeScript checking and production bundling are part of the release checks.

The test inputs are synthetic fixtures. They do not certify undocumented X response shapes or endpoint availability.

## Manual checks

The native release binary is exercised in a real terminal with `--demo`, using six fictional accounts. The workflow includes selection, cancellation, approval, fake removal and clean exit. The Chrome options page is visually inspected as a local file in an isolated browser, without using a signed-in X profile. Its extension-only Chrome APIs are not available in that local-file preview; this was a layout check, not an installed-extension integration test.

## Remaining live qualification

The user’s first live run collected following data, then failed on the first followers request. The v0.1.2 changes have not yet been qualified by a new live scan. Live compatibility remains unverified for the updated follower pagination, relationship lookup, replies/reposts coverage, `RemoveFollower`, and post-removal verification. Discovered operation IDs are capability candidates, not successful endpoint probes.

Before a real cleanup, verify the owner, collect a small inventory and inspect its evidence. A separately reviewed small removal trial must establish that native removal works and independently confirm the incoming relationship disappears. Private API failures and X restrictions may require changes in `extension/src/x-client.ts` or `parsers.ts`. The application deliberately stops or retains unknown evidence when it cannot establish these facts.

## Review changes

A focused reuse/quality/efficiency review removed duplicate action identity inputs/matches, avoided idle redraws and off-screen row allocation, corrected focus after filtering, avoided repeated credential storage, and allowed forced operation refresh. It also identified and fixed pause queue races, stale owner confirmations, account changes during verification, and incomplete activity evidence.

## Cleanup animation update

Added a display-only ladle/water/X sequence with owner, current target and the existing verified-removal counter. A monotonic clock drives bounded 10 FPS rendering only during visible active batches; paused/disconnected views are still. `--no-animation` disables moving frames. No work, timing policy, protocol or counter semantics changed.

Verification: `cargo test --offline --lib` passed before and after review (8 passed, exit 0); `cargo clippy --offline --all-targets -- -D warnings` passed before and after (exit 0). Controller tests passed (5 passed), release build and archive integrity checks passed. Real-terminal demo checks covered animated 90×26 output, pause, clean exit, and compact 52×14 output with `--no-animation`. No live X action was used.

Simplify review: reuse and efficiency passes were clean. Quality review removed the duplicate controls hint from `src/ritual.rs` and reused the shared footer in `src/ui.rs`, keeping cancel visible on compact layouts. No speculative changes were applied. The feature adds 194 net Rust lines; most of the `ui.rs` diff is nesting the existing table under the alternate work presentation.

## Terminal installer

The private-release installer supports authenticated download, SHA-256 verification, local/offline bundles, explicit version selection, stable extension paths, managed zsh/bash PATH setup, upgrades and uninstall. It preserves application data and rejects unmanaged destinations. Seven isolated installer integration tests pass, covering fresh install/upgrade/uninstall, checksum failure, archive traversal, version mismatch, unmanaged paths/executables and install locking.

Review fixes: uninstall acquires the existing exclusive installer lock; Bash setup selects the existing login startup file rather than shadowing it. Packaging builds a fresh archive tree in a temporary directory and never sweeps stale files from dist into the release. An initial review test overlapped a package rewrite and correctly rejected inconsistent checksums; tests were rerun sequentially after packaging and passed.

## Guided pairing (v0.1.1)

First-run setup walks through extension installation, copying local pairing details, and identifying the signed-in X account. The secret is hidden by default and can be copied with `y` on macOS or revealed with `v`. An authenticated bridge connection advances to the account check; a successful identity response advances to a confirmation screen. Enter opens the follower list without starting work. Shift+P reopens setup and pauses work. Previously paired users with a saved owner skip setup.

Verification: all 20 Rust tests passed, including four setup tests covering failed identification/retry, disconnected readiness, secret visibility, returning/reset/incomplete setup, narrow rendering, and suppression of cleanup dispatch. All 24 extension tests, TypeScript checking, Clippy, formatting and release compilation passed. A real-terminal smoke check at 94×26 exercised Install → Pair → Connect and clean exit with isolated data. The simplify review removed an impossible layout branch using Ratatui's array API and a redundant secret-visibility assignment (net −4 lines); the focused 17-test baseline and Clippy passed before and after. No live X account was scanned or modified.

The installed-binary smoke check caught macOS reporting the executable's symlink path. Setup now canonicalizes that path before resolving the bundled extension folder. The four setup tests and Clippy passed after the fix.

## X adapter hardening (v0.1.2)

Read-only inspection of the saved SQLite state confirmed 1,231 known following accounts, no checked activity, and failure at followers collection before its first cursor. The 129 follower relationships were mutuals present in that following inventory, not a completed follower scan. No action receipts existed. No authentication secrets were read from Chrome or copied into the terminal.

Compared 12 relevant GitHub repositories by stars, push date and applicability. Cloned the three highest-starred unarchived candidates pushed within one year into `references/`, pinned in `references/manifest.json`. The clones remain local research checkouts and are excluded from releases.

Changes:
- Parse current user verification, relationship counts and tweet counts; missing values remain unknown. Unknown verification can be enriched but cannot qualify for removal.
- Generate fresh path/method/time-bound transaction signatures in Chrome using a small attributed numerical port, validated against 12 deterministic vectors from the pinned independent Python reference. Signing ingredients come from the signed-in X tab and statically inspected X assets; downloaded code is not executed.
- Discover x-web/Vite and responsive-web/webpack assets, bounded relative imports, new webpack hashes and Relay query definitions. Observed per-operation features take priority over unrelated observations. Asset fetches omit credentials.
- Read-only GraphQL 404 recovery refreshes operation/signing data once. Persistent failures name the operation; 403/429 and mutations are not retried as stale queries.
- Inspect current Posts, Replies and Reposts channels from their first page. Positive recent activity protects immediately; negative evidence needs every channel. Foreign original-post dates do not establish repost time, and ambiguous activity stays unknown.
- Fail on missing graph pagination boundaries instead of reporting completeness. Identify the scan phase in the TUI and distinguish verified accounts from verification not yet checked.
- Reject old browser adapters and refresh old scan evidence on `s`, preserving keep choices and action history.

Validation: `cargo test --offline --locked` exited 0 (23 passed), `cargo clippy --offline --all-targets -- -D warnings` exited 0, `npm run check` exited 0, and `npm test` exited 0 (41 passed). Release compilation and Chrome extension bundling passed. Network regression tests use mocked responses, not a live account. The signing vectors validate the protocol port, not X’s current server acceptance.

Simplify review removed duplicated cancellation/relationship guards and a redundant completeness condition (net −9 implementation lines). Review also reproduced and fixed a CSRF-cookie cancellation race and inherited activity cursors; dedicated regressions now cover both. Initial focused baseline was 19 Rust tests plus Clippy and 39 extension tests; final suites retain those passes and add regressions. Broader cancellation of already-running asset downloads was left unchanged: batches are bounded and do not dispatch cleanup work.

A new live scan is required after reloading the extension. No follower removal was performed in this update.
