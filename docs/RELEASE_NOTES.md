forgive-me v0.1.8 makes follower cleanup a clear four-step workflow and adds background queues.

- s collects followers; i checks activity; f defines removal candidates and protections; a selects candidates and d reviews the full queue.
- The view and evidence panels state REMOVE or KEEP. Unknown evidence remains protected.
- The full approved selection stays queued. A local hourly attempt budget controls execution rather than truncating the selection.
- Endpoint cooldowns honor X reset and Retry-After headers, persist across Chrome worker restarts, and back off after repeated rate limits. Read failures and pre-dispatch deferrals preserve work; uncertain writes stop and never replay automatically.
- b hands the approved queue to a detached controller after the active task finishes. Reopen forgive-me for a monitor, or use status, pause, resume, and stop commands.
- Setup reports the Chrome extension version independently of the terminal version.

Quit and reopen the terminal app, Reload the installed extension in chrome://extensions, and refresh X. Existing settings, pairing, keep list, and action receipts are preserved. Chrome must remain open and the Mac awake for background execution. Automatic launch after a reboot is not configured.

The background behavior is tested with local simulated Chrome connections. No live followers were removed and no overnight live run is claimed.
