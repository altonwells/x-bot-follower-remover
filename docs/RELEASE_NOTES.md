forgive-me v0.2.0 simplifies cleanup to one rule and one approval.

- Remove followers with no posts in 30 days, who are not verified and whom you do not follow. No sparse-account or statistical override. Posts, replies and reposts count as activity; empty accounts must be at least 30 days old.
- Enter then y starts collection, checking, queueing and removal in a background worker. q closes the monitor without stopping the job. Space pauses; comma controls pacing and idle-sleep prevention.
- Repair activity parsing for conversation modules, empty channels, pagination and combined-timeline fallback. Inspection and approval use one cutoff timestamp.
- Persist retries and recover uncertain removals automatically. One unreadable account does not stop the pass. Failed-removal retry budgets survive reinspection. Authentication failures still pause work.
- Preserve saved approval through multi-day queues without fetching activity timelines again. Current verification and relationship protections are checked before removal.
- Preserve explicit pauses across process restarts. Older approved queues keep their original rules.

Install the update, reload the Chrome extension, and restart forgive-me. Stop an older background worker first with forgive-me stop. No live removals were run during development.
