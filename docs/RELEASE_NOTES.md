forgive-me v0.1.10 separates activity review from removal and improves sparse-account rules.

- Check activity with i, select cleared candidates with a, then approve with d and y. Removal uses saved activity evidence and makes no activity timeline requests. Identity and relationship protections still apply.
- New reviews allow sparse + old accounts: at most five posts and an observed activity date older than your inactivity cutoff. This can qualify an account with incomplete timeline coverage. Change the limit in f; 0 disables it. Existing approved queues keep their original rules.
- Incomplete checks say REVIEW, not KEEP. Dates say Seen to distinguish observed posts from complete inactivity evidence.
- Account evidence shows post-count percentile and log-scale z-score for at least 30 known counts. Low outliers and numeric handles are supporting review signals, not independent bot verdicts.
- Unchecked accounts cannot enter the removal queue. Saved activity older than 24 hours pauses for review instead of triggering a hidden rescan.

Restart forgive-me and Reload its extension in chrome://extensions before removing. The TUI checks for the new saved-activity capability and stops if the loaded browser worker is outdated. Pairing, settings, keep choices, and action history are preserved.

No live followers were removed during testing.
