# Browser boundary, version 1

`schema.json` documents the full wire contract. Rust serde types and TypeScript runtime validation implement it; policy fixtures are consumed by both test suites. Schema is a reference contract, not runtime code generation. Demo identifiers deliberately bypass the network contract.

1. Extension opens `ws://127.0.0.1:47831/bridge` with its Chrome extension origin.
2. First message: `{type:"hello",v:1,token,extension_id}`. Authentication has a 5-second deadline. The token is not in the URL.
3. Controller sends `{type:"welcome",v:1,session_id}` and requests `get_session`.
4. Each command envelope carries `v`, `session_id` and `work:{command_id,owner_id,command}`. Commands use a `kind` tag. Results carry the same session and command ID.
5. Ten-second heartbeats maintain MV3 worker activity. Missing activity for 35 seconds closes the connection. Only one authenticated connection is served at once; messages/frames are limited to 1 MiB.

The extension can send `{type:"x_page_ready",session_id}` after an X page loads. This does not reconnect or authorize work. During setup only, the controller requests identity again if idle. If a check is running, it can retry once after a recoverable failure; successful checks, access denials, account changes, and rate limits do not trigger that queued retry. Update both components together; older terminals do not recognize this notification.

Owner and target IDs are strings. Work identities and session IDs are random 256-bit hexadecimal values. Browser commands are a fixed allowlist: session, scan page, inspect account, native remove follower, reconcile, open profile. No arbitrary URLs, JavaScript or shell execution are accepted.

The controller owns the queue. Extension execution is serial; pause/cancel controls bypass the queue and invalidate both queued and in-flight preflight generations. Resume permits new work, never revives a previously cancelled preflight. A mutation already dispatched can finish after pause.

SQLite stores normalized accounts and scan cursor together. Removal attempts are persisted before the command is sent. The extension writes an accepted marker, then a conservative dispatched marker, then its final receipt. If it crashes between the marker and the actual request, reconciliation is still required; uncertainty is preferable to repeating a possible write.

The controller acknowledges only after committing the result. The extension keeps uncertain receipts until an explicit `reconcile` resolves them; reconciliation checks relationships without performing a write. A confirmed old target still following the owner closes the old attempt as failed, requiring a fresh reviewed batch for another attempt. Terminal database outcomes cannot be downgraded by a stale recovery receipt.

Reads can be repeated after interruption. A deferred receipt certifies that the removal was not dispatched; it includes the target and a retry deadline. The controller can retain that target and issue a fresh command after the cooldown. Any write that may have dispatched is never automatically replayed. On disconnect work stops; saved queues retain exact target IDs and policy. An explicitly detached queue can resume only after re-identifying the same owner and confirming there are no uncertain receipts. Account rebinding clears confirmation and selection, and data remains partitioned by owner.

Durable queue support is advertised as durable_queue:1. The hello message can include extension_version so the UI can distinguish the browser build from the terminal build. The legacy policy field batch_limit now limits attempts in a rolling hour; it no longer truncates the approved selection. Cooldowns are persisted separately by owner in SQLite and by owner/endpoint in Chrome local storage.

As of v0.1.10, saved_activity:1 advertises removal without an activity rescan. RemoveFollower carries approved_account: the persisted activity result for that target. The TUI approves only cleared accounts and sends the evidence with the frozen batch policy. Both sides require evidence no older than 24 hours. The browser rechecks identity and current relationship/verification/visibility protections, then removes without fetching posting timelines. Missing or expired evidence stops work for review. sparse_old_max_posts is optional (missing means disabled on old approved queues); future reviews default to 5. This rule accepts old observed activity plus a low post count without claiming complete coverage.


## Single-rule jobs (v0.2.0)

`simple_cleanup:1` advertises the 30-day rule and durable receipt handoff. New job policy has `simple_cleanup:true`, `inactive_days:30`, and both following/verification protections enabled. Old policies deserialize simple_cleanup as false and keep their existing behavior. No statistical or sparse-account override is used for new jobs. Account evidence can include created_at_ms; zero-post accounts need a known creation date at least 30 days before the check.

Inspection uses one timestamp for checked_at_ms and the activity cutoff. Queue eligibility is checked when evidence arrives. At removal, simple jobs evaluate saved activity against that recorded timestamp, so approved work does not expire during a multi-day queue. Identity, relationship, verification and visibility are read again; activity timelines are not.

For new jobs, an authenticated ack may include durable:true after the controller has committed the action outcome to SQLite. Chrome can then release its single receipt slot. SQLite remains authoritative for uncertain targets: a target with an unresolved action cannot be inspected or removed again until a read-only reconciliation closes that action. Other targets can proceed. Legacy acknowledgements retain the old Chrome receipt behavior.

Retries are persisted per owner and target with attempts and due_ms. Inspection success does not reset a failed-removal retry budget. Confirmed absence or exclusion closes it. A confirmed-present result may schedule a new bounded attempt with a new command ID and the previously approved evidence; the unresolved command is never replayed. After four failures the target is set aside until a later approved pass.

The worker owns the job from approval. The TUI sends fixed commands over a mode-0600 Unix socket. Processing-setting updates copy only pacing fields; they cannot weaken approved eligibility. managed:{owner} preserves explicit pause across process restarts. Normal same-owner Chrome reconnect may resume a running job; account switches and authentication failures stop it. Uncertain results are recovered without requiring a global manual reconciliation step.
