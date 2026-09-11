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

## Workflow and background queues (v0.1.8)

The user confirmed that loading the updated extension resolved the connection failure. The browser-transport replacement proposal was not adopted. The current signed-in adapter remains in use.

Collection now stops at a review boundary; i starts activity checks. REMOVE and PROTECT labels describe candidate rules, and m explicitly changes only the view. Approval includes every selected eligible target. The existing batch_limit field now enforces a rolling hourly attempt budget rather than truncating the queue. New defaults are a 60-second minimum interval and 50 attempts per hour; saved settings are preserved. Existing approval, keep exceptions, and per-target fresh evidence checks remain.

Reference inspection: twscrape's QueueClient/AccountsPool preserves endpoint reset locks; twikit's TooManyRequests carries the server reset header. Sources: https://github.com/vladkens/twscrape/blob/main/twscrape/queue_client.py and https://github.com/d60/twikit/blob/main/twikit/errors.py . X documents reset/remaining headers at https://docs.x.com/x-api/fundamentals/rate-limits ; those public API quotas are not assumed to apply to the private web interface. No account rotation or attempts to evade limits were added.

Chrome persists owner/endpoint cooldowns, honors reset and Retry-After, and applies bounded exponential backoff when necessary. The controller persists hourly attempt reservations and cooldown timestamps. A deferred receipt is emitted only before the dispatch journal boundary. It closes that attempt without removing the target from the queue. Possible writes remain uncertain and cannot be replayed. Queued targets with old evidence are freshly checked rather than silently discarded for age alone.

The same App controller can run as a detached child with separate process group and redirected stdio. It holds the original process lock and Chrome bridge; a mode-0600 Unix socket exposes only fixed local controls and status. Reopening forgive-me monitors the worker. Handoff waits for the pending browser task. Normal reconnect can resume only the same approved owner; manual pause, owner change, missing adapter capability, and uncertain receipts stop automatic continuation. Automatic launch at login is not configured, and Chrome/Mac availability remains required.

Simplify review removed a duplicated success branch (−2 lines) and suppressed full redraws during idle cooldowns (no net line growth). Review also caught a page-ready event clearing an identity retry; the retry is now retained across unrelated events. The regression waits through the real 30-second backoff before observing a new session command. Completed/cancelled queues report no queued work instead of a misleading pause. The three review passes found no further actionable simplifications.

Checks: full Rust suite exited 0 (44 tests); extension TypeScript check and 59 tests exited 0; Clippy with -D warnings exited 0. Release compilation/bundling passed. The installer suite passed 7 tests and the release PTY suite passed 1. The background integration verifies a private control socket, manual-pause preservation across reconnect, and page-ready retry survival. Rendered dashboard and rule modal were visually inspected. A separate packaged-process smoke test uses an isolated data directory and no X connection. Its first sandboxed run was denied a local socket bind and was rerun with socket permission.

These checks do not constitute a live overnight removal run. No real followers were removed during development.

The packaged-process smoke test passed after granting local socket access (Ran 1 test, OK). It verified status, pause, stop, and socket cleanup in a separate process with no X connection.

## Ordered activity checks and basic selection (v0.1.9)

Activity dispatch, the inventory, and new queue approvals use the same case-insensitive handle order with an ID tie-breaker. Starting activity checks clears search and candidate-only view so the target remains visible. Focus follows dispatched work; pending command state supplies the row marker and action label. The list stays visible during a queue, with v switching to the existing animation.

Shift+A selects current-view accounts that pass the shared basic protection checks without requiring completed activity evidence. Lowercase a retains fully checked eligibility. The browser's fresh inspect/eligible gate before removal is unchanged. Confirmation now explicitly approves conditional removals; selected accounts with insufficient activity evidence show CHECK FIRST.

New controller regressions use account IDs in the opposite order from handles, verify the first and second dispatch plus focus movement, and cover basic selection before activity, view scope, protections, approval order, and first removal dispatch. Presentation tests cover active work, CHECK FIRST, list/animation switching, and warnings/default cancel at 52×12. Offscreen fictional queue and minimum confirmation previews were rendered and inspected.

Checks exited 0: cargo test --offline --locked (47 Rust tests); npm run check and npm test (59 extension tests); cargo clippy --offline --all-targets -- -D warnings; release build/package; installer suite (7), release terminal suite (1), and packaged background CLI suite (1). Socket-dependent tests failed under sandbox restrictions, then passed with local socket permission. Total: 115 tests. No live followers were removed.

Simplify's independent reuse, quality, and efficiency passes found no worthwhile simplification. No cleanup edits or skipped actionable findings; cleanup net line delta 0. The focused baseline (controller, presentation, policy contract: 22 tests) and final full-suite coverage passed unchanged, exit 0.

## Sparse-account rules and saved activity (v0.1.10)

The supplied screenshot combined an old observed post with incomplete coverage in another activity channel. This is not confirmed full inactivity. The new explicit sparse + old rule permits a known public, unverified, non-mutual, non-kept follower with 1–5 current posts and an old observed activity date to qualify despite incomplete coverage. Recent observed activity overrides the sparse and zero-post rules. The limit is adjustable, and 0 disables it. Existing approved batch policies deserialize the missing field as disabled; migration enables the new default only for future reviews. Shared Rust/TypeScript fixtures cover boundaries, missing dates, recent activity, staleness, and retained protections.

At the user's request, removal no longer calls inspect. Approval accepts only saved evidence that already clears the frozen policy. RemoveFollower carries that account evidence; the browser checks it, checks current identity/relationship/verification/visibility, then writes. No activity timelines are requested. Evidence older than 24 hours stops for review. The new saved_activity:1 capability prevents dispatch to an older extension that would rescan activity. Unit and adapter tests cover missing/expired evidence, no activity requests, current protections overriding saved clearance, and mutation uncertainty. New activity after the saved check may not be detected by this workflow.

Post-volume statistics use Welford sample variance on log(1 + posts), a midrank percentile for ties, and a minimum of 30 known counts among collected followers. A log z-score ≤−2 is labeled LOW OUTLIER. No normality, bot probability, or completeness of the follower population is assumed. Counts, statistics, and numeric handles are review signals; the explicit sparse rule controls eligibility. Reference: [NIST outlier guidance](https://www.itl.nist.gov/div898/handbook/eda/section3/eda35h.htm) and [NIST skewness/transformation guidance](https://www.itl.nist.gov/div898/handbook/eda/section3/eda35b.htm).

Simplify's three independent passes found one display issue: optional statistics could displace activity evidence at shorter heights. Evidence now comes first, and details include the statistics. This reordered existing display work (cleanup net 0 lines); no speculative caches were added. The focused Rust baseline and final suite exited 0; final full Rust suite passed 51 tests. Extension typecheck and all 62 tests exited 0. Clippy with -D warnings and the release build/package passed. Installer tests passed 7, release terminal test passed 1, and packaged background process test passed 1 (122 total). Fictional sparse-account, queue, and minimum-size approval previews were rendered and inspected. No live follower removals were run.


## Processing settings, Full Auto, and continuous animation (v0.1.11)

System Settings controls the actual removal interval, attempts per batch, batch rest, and rolling hourly attempt cap. The recommended starting configuration is 60 seconds, 20 attempts, 300 seconds of rest, and 50 attempts per hour. These are local defaults, not published X quotas. Saving copies pacing into the current approved queue and Full Auto policy without changing approved eligibility or shortening an existing cooldown. Attempt reservations, rest progress, and cooldown timestamps persist. Existing server reset/Retry-After and bounded backoff handling remains in use.

Full Auto requires explicit approval and starts fresh relationship collection even from an existing review. It checks candidates in display order, stores each activity result with its eligible queue entry in one transaction, and removes using saved activity evidence. It stops after one complete pass. Verified accounts, people followed by the owner, keep exceptions, protected accounts, unknown required evidence, and recent observed activity retain the shared policy protections. The existing sparse + old rule remains explicit; post-volume statistics remain review signals. No second activity timeline check is added at removal.

Batch and Full Auto state save together. Pausing survives a late final collection response; cancelling prevents a late activity result from creating new automatic work. Background handoff can resume an approved Full Auto collection before any removal batch exists. Crash/restart opens paused in the TUI. Chrome and the computer must remain available for background work; this release was not tested in a live overnight run.

The display clock runs independently at 20 frames per second with missed frames skipped. Water continues through cooldowns, confirmed removal tags move downward, and pauses stop the animation. A bounded eight-tag history receives only confirmed removals. The logo is drawn above the water to remain legible. Offscreen fictional cleanup, System Settings, and minimum-size confirmation previews were rendered and inspected.

Simplify completed independent reuse, quality, and efficiency reviews. The isolated dead status branch cleanup removed 2 lines in src/background.rs. Additional correctness fixes reuse start_scan and Store::save_page, remove the separate inspection checkpoint write, and save batch/auto state atomically. They fix fresh-pass startup, activity/queue durability, and late-response pause preservation. The zero-post confirmation label now reflects its setting. No speculative abstraction or dependency was added; no actionable findings were left open. The pure cleanup delta is -2 lines; the functional fixes and regression tests are included separately in the feature diff.

Baseline focused verification: cargo test --offline --test controller --test presentation exited 0 (20 controller and 10 presentation tests). After the review fixes, the same command exited 0 (22 controller and 10 presentation tests). Final cargo test --offline --locked exited 0: 59 Rust tests passed. npm run check and npm test exited 0: 62 extension tests passed. cargo clippy --offline --all-targets -- -D warnings exited 0. After the final drawing adjustment, presentation tests again reported 10 passed and Clippy exited 0. Release compilation and extension bundling passed. Installer tests reported Ran 7 tests, OK; release terminal and packaged background CLI tests each reported Ran 1 test, OK. Total: 130 tests. The background smoke test used an isolated data directory and permission to bind local sockets. No live X account actions were performed.
