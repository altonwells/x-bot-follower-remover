# X adapter references

Checked September 10, 2026 using GitHub repository metadata and default-branch commits. Selected the three highest-starred projects from the comparison that were unarchived and pushed within the last year. Stars measure adoption, not proof that a particular X operation works. Twikit has the most stars but its last update is six months old; the August references are the stronger evidence for recent X changes.

| Repository | Stars | Last default-branch commit | Role |
|---|---:|---|---|
| [d60/twikit](https://github.com/d60/twikit) | 4,662 | 2026-03-10 | Request/session and legacy GraphQL comparison |
| [vladkens/twscrape](https://github.com/vladkens/twscrape) | 2,758 | 2026-08-28 | Signing, Vite/webpack discovery, modern user fields |
| [prinsss/twitter-web-exporter](https://github.com/prinsss/twitter-web-exporter) | 2,694 | 2026-08-31 | July user schema and August activity endpoint changes |

Also compared: twitter-api-client (1,894 stars, last push 2024-05-22; stale), TwitterInternalAPIDocument (713, 2026-09-10), twitter-scraper (639, 2026-04-01), twitter-defollower (9, 2025-01-22; stale), xbotremover (5, 2026-08-15), x-bot-cleaner (2, 2025-11-22), Admuad/x-account-cleaner (1, 2026-09-01), X-Cleaner---Followers-Following (0, 2026-06-21), and goawaynow (0, 2025-09-30). These are the search pool, not a claim to rank every X repository on GitHub.

`manifest.json` pins each clone and records its URL. Clones sit beside this file, retain their own Git histories/licenses, and are ignored by the app repository. They are not shipped, imported, or run against X. To recreate, clone each repository into its named folder and check out the manifest commit.

## Findings used in remover

- `twitter-web-exporter/src/types/user.ts` and `src/utils/api.ts`: verification moved to `verification.verified`; counts to `relationship_counts` and `tweet_counts`. Missing values must stay unknown for cleanup decisions.
- `twitter-web-exporter/src/modules/user-tweets/api.ts`: August migration splits posting into `UserOriginalsTimeline`, `UserRepliesTimeline`, and `UserRepostsTimeline`. Negative inactivity evidence must cover all three, and original-post dates cannot date reposts.
- `twscrape/twscrape/xclid.py` and `scripts/update-gql-ops.py`: current assets may use `/x-web/`, relative imports, Relay operation definitions, and changed webpack hash lengths. Request signing uses freshly generated path/method/time-bound transaction IDs, not replayed captured headers.
- [twscrape issue #322](https://github.com/vladkens/twscrape/issues/322): Followers may return an empty 404 while Following works when transaction signing is invalid. This matches the reported failure pattern; the saved controller state confirms failure during followers collection, but no original HTTP trace was captured.

The numerical signing port is credited in the application's third-party notices. Synthetic signing vectors are generated from the pinned twscrape numerical routines. Other references inform independently written parsing, discovery, and controller changes.
