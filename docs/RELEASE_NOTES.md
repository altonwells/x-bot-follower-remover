forgive-me v0.1.7 repairs missing X request-signing data.

- If X has removed its loading SVGs from the live page, read the original home HTML inside Chrome. Read the verification key and animation frames from the same response.
- Discover bare relative signing script names, including sign.o and ondemand.s assets. Accept JavaScript variable names that contain a dollar sign.
- Report the missing signing ingredient and asset counts instead of a generic signing error. Preserve rate-limit cooldowns and stop before API dispatch if signing is unavailable.
- Keep existing pairing, account confirmation, and removal approval behavior.

Installed users: quit and reopen forgive-me, Reload the extension in chrome://extensions, then refresh the signed-in X tab. Press Enter to retry the account check if needed.

Verified signing generation against current public X HTML and JavaScript assets. Regression tests exercise the serialized Chrome collector and account identification with synthetic API responses. Authenticated X acceptance remains unverified; no live account was scanned or modified.
