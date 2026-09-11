# Use the signed-in X page to do the work

Status: not adopted. The user confirmed that loading the correct extension resolved the connection failure. v0.1.8 retains the signed-in API adapter and improves its workflow and durable queue. The design below is retained as a research alternative, not a description of the installed implementation.

## Purpose

Remove unwanted followers with the existing terminal controls and Chrome session. Setup must not depend on reproducing X's private request signer. Keep the TUI, local database, pairing, filters, keep list, approval flow, and action journal.

The current session check requires asset discovery, signing preparation, and a custom profile request. A failure in any of those prevents account connection. Fixing that check alone would leave the same dependency in scanning and removal. Replace the browser transport as a unit.

## Evidence

- twitter-web-exporter's `src/core/extensions/manager.ts` installs an XHR response hook in the page context. Its Following interceptor collects data from requests that X already made. Pinned source: ddf2b0c823680b904daae166f3eb05b81950ce7e in `references/manifest.json`.
- x-bot-cleaner's `attemptRemoval` finds a follower cell, opens its More menu, selects Remove this follower, and confirms Remove. It then waits 800 ms and declares success. Use the UI interaction approach, not that success criterion. Public source: https://github.com/iuzn/x-bot-cleaner/blob/main/src/pages/content/followers/controller.ts
- Chrome supports an isolated content script for DOM interaction and communication with the extension. Response observation must run in the page's MAIN world because isolated scripts cannot replace the page's JavaScript globals. See https://developer.chrome.com/docs/extensions/develop/concepts/content-scripts .

## One controlled X tab

Bind each job to one tab, one document/navigation generation, and the confirmed numeric owner ID. Keep that tab visible while it works. Opening routes and scrolling cause X's own application to make requests. Do not fetch GraphQL endpoints or download signing assets in the new transport.

Use a small packaged MAIN-world observer installed before X starts. Observe both fetch and XHR without modifying their requests or original responses. Accept only the account, follower/following, activity, and removal operations needed by the active job. Forward bounded response evidence to an isolated content script, then to the extension worker. Do not forward cookies, authentication headers, unrelated timeline content, or messages. Hooks must preserve errors, response bodies, and receiver binding for X itself.

Only the authenticated local controller can start work. Page observations are data, never commands or approval. Validate tab, frame, document, operation, target ID, response shape, freshness, and job generation before accepting evidence. A page message cannot authorize a removal.

## Connection

1. Pair Chrome as today.
2. Locate the signed-in account's own profile link in X's navigation. A viewed profile or mention is not account identity.
3. Open that profile through X's normal navigation and observe its normal profile response.
4. Require the response's numeric ID to match the browser session's existing owner ID check, and require the handle to agree with the own-profile navigation link. Check owner again before accepting.
5. Ask for account confirmation in the TUI as today.

No signing preparation is needed. Show sign-in, page loading, identity mismatch, and unsupported page as separate states. A successful identity check does not claim that scan or removal capabilities have been proven.

## Collection and activity

Open the confirmed owner's follower or following route. Capture each response X requests as the extension scrolls the correct list. Deduplicate by numeric ID and persist pages as they arrive. Bind each response to its list owner, operation, requested cursor, and active navigation. Repeated or out-of-order responses cannot advance the cursor chain.

A stalled scroll, empty DOM, or missing cursor is not proof of completion. If X does not expose a recognized end condition, preserve partial progress and stop with an incomplete result. Saved cursors describe captured evidence; they are not replayed through custom API calls after restart. Resume by revisiting and deduplicating the list.

Inspect candidate profiles through normal navigation. Read the structured responses X already uses for profile facts and Posts/Replies/Reposts. A recent action can protect immediately. Inactivity requires the existing coverage rule across all required channels. Missing channels, hidden activity, and ambiguous verification remain unknown. Do not label unknown accounts as bots. Keep the current AND filter and keep-list behavior.

## Removal

For each approved target:

1. Check the current owner, target numeric ID, fresh relationship/activity evidence, policy, and keep list.
2. Locate the target's own profile/list controls and open its More menu. Use scoped controls; do not click a global first match.
3. Select Remove this follower and validate the dialog and target context.
4. Recheck the controller generation, owner, target, and deadline. Persist the dispatch journal before the final click. A pause, disconnect, or changed target cancels pending clicks.
5. Click the final confirmation once. Let X create and send its request.
6. Match the resulting removal response to the approved target and verify the relationship through a fresh page read. A disappeared DOM row or elapsed delay alone is not success.
7. If the final click may have dispatched and evidence is missing, retain an uncertain receipt. Never retry that write automatically.

Initially support the observed English UI. If controls or language differ, stop with an actionable error rather than guessing. Maintain bounded waits and existing pacing. UI automation remains subject to X limits and UI changes.

## Implementation boundary

Add a BrowserClient implementing the existing Runner Client contract. Reuse parsers and eligibility checks after validating observation envelopes. Keep the current XClient available only for comparison during development; do not automatically fall back to custom API writes. Route all production work through one chosen transport for a session.

The new implementation needs a packaged page observer, an isolated tab driver, and the BrowserClient coordinator. It does not need Playwright, a separate Chrome profile, an LLM agent, a new server, or a cloud service. Existing Chrome permissions should be reviewed against the final implementation; no debugger permission is planned.

The Rust controller must report connection separately from operation readiness. Capability reporting must update when the browser proves a route or control is available. Do not advertise remove_follower solely because the user profile loaded.

## Proof before release

First build a read-only probe in the real signed-in Chrome tab, with the custom signer and direct API transport disabled:

- Identify the owner and handle using the navigation/profile response match.
- Capture at least one follower page requested by X, bound to that same owner.
- Open one candidate profile and receive structured account/activity evidence.
- Repeat after refresh; verify pause and owner-change rejection.

These are live acceptance gates, not mock substitutes. Only after they pass should the replacement be connected to the full scan controller. Exercise removal dialog targeting without confirming during development. A real removal requires a specifically approved target; it is not part of the read-only probe. Release only with a precise record of which live gates passed and which remain unverified.

Automated regressions still cover observer transparency, response limits, wrong-tab/owner/target rejection, stale navigation, incomplete pagination, unknown activity, pause-before-click, crash-after-click, and uncertain receipts. They support the live proof; they do not replace it.
