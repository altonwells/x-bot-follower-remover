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

## Visual and fullscreen update (v0.1.3)

Rebuilt the TUI presentation with an explicit full-screen palette, responsive dashboard, live evidence inspector, refined setup and review panels, and the display-only cleanup ritual. No eligibility, queue approval, persistence, browser requests or protocol semantics changed. Help/detail scroll offsets are clamped to rendered content. Small confirmation dialogs keep both the irreversible-action warning and default-cancel controls visible. Motion remains bounded at 10 FPS and animated frames avoid follower-inventory traversal.

Ratatui already used the alternate screen. This update adds mouse capture so wheel input stays in the app, routes wheel events only to navigation, rejects noninteractive output, redraws on resize, and releases capture on normal/error/panic exit. The PTY test emulates cursor-position queries, checks alternate-screen and mouse enable/disable sequences, scrolls the inventory/help, resizes from 120×34 to 52×12 to 140×42, exits with q, and verifies no newline-based scrolling output.

Rendered fictional fixtures with Ratatui's TestBackend and an offscreen AppKit renderer; visually inspected dashboard, compact/minimum layout, removal review, policy, pairing, and cleanse scenes. Preview generation is reproducible through `examples/preview.rs` and `scripts/render-preview.swift`. Ratatui remains pinned to 0.30.2; its rendered-line-info feature is used to clamp wrapped dialog content precisely. No new runtime dependency was added.

Verification: full `cargo test --offline --locked` exited 0 (27 passed). The sandbox initially denied localhost binds in two bridge tests; rerunning with local socket permission passed. Focused baseline and final `cargo test --offline --lib --test controller --test setup --test presentation` both exited 0 (24 passed), and `cargo clippy --offline --all-targets -- -D warnings` exited 0 before and after review. Extension `npm ci --offline`, `npm run check`, and `npm test` exited 0 (41 tests passed).

Simplify review: reused `theme::centered` in setup and removed an unnecessary manual resize clear already performed by Ratatui (net −6 lines, one fewer terminal I/O call per resize). Quality review was clean. An additional Details inventory traversal was left alone because eliminating it required broader borrow/parameter restructuring. No live X account was scanned or modified.

Release compilation and extension bundling passed. `python3 tests/install_test.py` exited 0 (`Ran 7 tests`, `OK`); `python3 tests/terminal_test.py target/release/forgive-me` exited 0 (`Ran 1 test`, `OK`). The PTY lifecycle check used the optimized release binary.

## Automatic Chrome pairing (v0.1.4)

The terminal guide opens Chrome's extension page and copies the installed folder path on `b`. Chrome still requires the user to load the unpacked extension. The new native messaging helper transfers the existing port and secret over a private standard-input/output pipe. It checks the caller's exact extension origin, request shape, live TUI lock, and saved identity before returning credentials. TUI startup registers the host for the active absolute data path. Uninstall removes only its own registration.

References: [Chrome distribution rules](https://developer.chrome.com/docs/extensions/how-to/distribute), [native messaging protocol and host registration](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging), and [Chromium extension path identity implementation](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/components/crx_file/id_util.cc). The canonical installed extension path retains its existing Chrome ID; no manifest key or identity migration was introduced.

The extension presents terminal connection, X sign-in, and account confirmation as separate steps. Manual settings remain available. Automatic pairing does not start a scan, approve an account, resume work, or approve removals. X cookie handling, the cleanup protocol, and action verification remain unchanged.

Verification: `cargo test --offline --locked` exited 0 (31 passed), including the compiled native host's framed exchange and authenticated local WebSocket connection. The native host integration test also passed from an isolated bundle, without a pre-existing npm build. `cargo clippy --offline --all-targets -- -D warnings`, `npm ci --offline`, and `npm run check` exited 0. `npm test` exited 0 (`tests 49`, `pass 49`, `fail 0`). Four new worker regression tests cover manual/automatic races, cancellation followed by reconnect, persistent manual endpoints, and an in-progress storage write.

The focused Rust baseline and final check each passed 27 tests. Simplify review removed redundant response type casts (net 0 lines). No shared helper replaced the native reader: the existing configuration loader can create missing settings, which is inappropriate for the read-only pairing host. Separately, correctness fixes removed shared in-flight connection coalescing, preserved manual settings during retries, serialized settings writes, and resolved relative data paths before Chrome starts the helper.

The installer suite passed (`Ran 7 tests`, `OK`), including ownership checks for native-host removal. The PTY test initially caught a test timing race: terminal EOF can precede the process becoming waitable. A bounded wait now checks exit without changing app behavior. The unchanged test also passed on retry. The corrected test passed (`Ran 1 test`, `OK`) and checks the same fullscreen, wheel, resize, and restoration behavior.

The updated pairing screen was rendered and visually inspected with fictional fixture data. Live Chrome installation, native-host launch by Chrome itself, and live X compatibility remain unverified. No live account was scanned or modified.

## Setup flow correction (v0.1.5)

Replaced manually advanced setup pages with live connection checks. Enter runs the displayed action; it cannot skip pairing or identity checks. Disconnect returns to pairing. Reopening setup shows the current connection state. A saved handle without a live browser connection cannot claim readiness. Account confirmation remains explicit and does not start work.

The centered setup panel uses a fixed primary action, live terminal/Chrome/account status, direct install/reload access, and concise instructions. At 52×12 the action and repair keys remain visible, the scroll hint appears when needed, and scroll offsets clamp on resize. The 120×34 and 52×12 previews were rendered and visually inspected.

`cargo test --offline --locked` exited 0 (34 tests passed). `cargo clippy --offline --all-targets -- -D warnings`, `npm ci --offline`, and `npm run check` exited 0; `npm test` reported `tests 49`, `pass 49`, `fail 0`. Three new setup regressions cover navigation/state gating, waiting during identity checks and reopening setup, and scroll clamping across resize.

Simplify review: reuse and efficiency found no worthwhile abstraction or performance change. Quality replaced misleading automatic-pairing text in the manual repair path with neutral guidance (net 0 lines). The compact scroll hint and scroll clamping were handled as separate usability fixes. The focused baseline passed 29 tests; the final focused suite passed 30, including the added scroll test. Clippy exited 0 before and after review. Live Chrome interaction and live X compatibility were not exercised; no account was scanned or modified.

## X account check interruption (v0.1.6)

The automatic pairing update added a page-completion listener that called connect again while accountHandle was empty. This closed the authenticated bridge and cancelled the browser reader, including an in-progress account discovery/profile check. The new worker regression failed before the fix (`2 !== 1`: two sockets instead of one) and passes afterward with one authenticated socket plus an x_page_ready notification.

Page-ready notifications are validated against the bridge session. During setup only, the terminal requests identity if idle; while identity is running it coalesces notifications into one queued retry. Success, rate limits, access denials, and account changes do not trigger that queued retry. Reconnect uses the same cooldown guard as manual retry. No page event starts cleanup. Account failures now retain their actual error code and message in a dedicated panel while Chrome stays paired.

History comparison also found that the pre-hardening session implementation swallowed most profile errors and substituted a numeric owner ID for the handle. That fallback was removed during X adapter hardening. It was not restored: a reported connection must identify the account successfully.

An operation-specific discovery optimization was considered and removed during review because the TUI's removal availability depends on the complete capability snapshot. The final X request, discovery, eligibility, and removal implementations are unchanged in this release. Simplify review reused check_session for initial reconnect as well as manual and queued retries (net −1 line at the reconnect call site). Quality tests cover cooldown preservation; no broader state abstraction was added.

References consulted: [Chrome tabs events](https://developer.chrome.com/docs/extensions/reference/api/tabs) and [script injection behavior](https://developer.chrome.com/docs/extensions/reference/api/scripting). These document page events and script execution, not live compatibility with X's private interface.

Verification: the unchanged extension baseline passed 49 tests; the new regression failed against the original listener and the final suite passed 50. npm ci --offline and npm run check exited 0. The final Rust suite passed 37 tests, including three new setup tests and an authenticated bridge round trip for x_page_ready. cargo clippy --offline --all-targets -- -D warnings exited 0. The account-error preview was rendered and visually inspected with synthetic data.

The user has not yet supplied the exact live X error. The reproduced reconnect defect is confirmed; whether an additional API, signing, or authentication failure affects this account remains unverified. No live browser state or X account was changed by testing.

Final release inspection corrected an accidental version-string substitution in the Chrome type dependency lock entry. Its version and URL now agree with the installed package and unchanged integrity hash (0.1.43). Future release versions must update only the package's own version fields.

## Signing-page recovery (v0.1.7)

The user's screenshot now identifies signing_unavailable as the account-check failure. Comparison with the pinned twscrape and twikit implementations found that they obtain the verification key and four animation SVG paths from the original home HTML. Our collector relied only on the hydrated page. The collector now fetches and inertly parses X home inside Chrome when the live page has lost those ingredients. It never combines a new HTML key with old page frames. Login-like responses stay unusable, HTTP 429 preserves a cooldown, and missing ingredients stop asset and API requests.

Static discovery now accepts bare relative signing filenames and byte-index variables containing a dollar sign. Detailed diagnostics report the source, ingredient presence, and successful/failed asset counts without exposing keys, cookies, or raw HTML. Existing action authorization and account identity checks remain in force.

The regression executes the serialized collector in a separate JavaScript context with an HTML parser, then exercises the real adapter through account identification using a synthetic API response. It covers a hydrated page with no SVGs, a fresh original HTML response, a bare signing chunk, and the generated transaction header. Linkedom is a development-only test dependency; it is not included in the Chrome runtime bundle.

A separate read-only probe fetched current public X home HTML without account cookies, then used the actual asset discovery and signing modules. It found all four SVGs, fetched four allowed static assets without errors, extracted four indices from ondemand.s.d213e36fda69a590a.js, and generated a transaction ID. The probe did not send an authenticated API request. This validates current public asset parsing and signing generation, not X's acceptance for the user's signed-in session.

References: [twscrape signing implementation](https://github.com/vladkens/twscrape/blob/main/twscrape/xclid.py) and [XClientTransaction example](https://github.com/iSarabjitDhiman/XClientTransaction/blob/master/quickstart.py). Existing MIT attribution is retained.

Simplify review removed duplicate seed validations and a derivable failed-asset counter, and skips futile asset downloads when the seed is unusable (net −5 implementation lines). Baseline and final extension checks each passed 54 tests; TypeScript exited 0. No speculative refactor was added.

Release checks: cargo test --offline --locked exited 0 (37 passed); cargo clippy --offline --all-targets -- -D warnings exited 0. npm ci --offline and npm run check exited 0; npm test reported tests 54, pass 54, fail 0. The release build and extension bundle passed. The installer suite reported Ran 7 tests, OK; the release PTY suite reported Ran 1 test, OK. The added no-download assertion initially included the fixture's successful setup requests; clearing those before removing the seed corrected the test without changing implementation.
