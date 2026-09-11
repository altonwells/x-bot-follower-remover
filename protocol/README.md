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

Reads can be repeated after interruption. Removal commands are never automatically retried. On disconnect the controller pauses; saved batches retain exact target IDs and policy. Account rebinding clears confirmation and selection, and data remains partitioned by owner.
