forgive-me v0.1.11 adds System Settings, Full Auto, and a continuous cleanup animation.

- Press comma for actual queue controls: removal interval, attempts per batch, rest between batches, and hourly limit. Shift+R selects the recommended starting configuration: 60 seconds, 20 attempts, 5-minute rest, 50 per hour. Current cooldowns finish first; X limits always apply.
- Shift+F reviews Full Auto for the entire follower list. After approval it collects fresh relationship lists, checks accounts in order, and removes eligible accounts using saved activity evidence. Verified accounts, people you follow, keep exceptions, and recent activity remain protected. One complete pass then stops.
- Full Auto and batch/rest progress persist. Pause, cancel, reconnect, and background handoff use the existing controller. Activity results and automatic queue entries save in one transaction.
- The animation pours continuously through cooldowns. Confirmed accounts float down the stream in small REMOVED tags. Unconfirmed attempts never add removal tags or counts.

Restart forgive-me to use the update. Reload the bundled Chrome extension to show the matching version. Open sessions are not terminated by installation. No live follower removals were used to test this release.
