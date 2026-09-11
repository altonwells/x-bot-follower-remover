# Releases

The installer downloads public GitHub release assets from `altonwells/x-bot-follower-remover` over HTTPS without authentication. Release checksums detect corrupted downloads; they are not independent signatures.

The first prebuilt target is Apple Silicon macOS. Build on that target with Rust 1.88+, Node 22+, npm and Python 3. No development tools are needed by installed users.

1. Update the version in Cargo.toml and extension/package.json / manifest.json when making a new release; regenerate the lockfiles as appropriate. In package-lock.json, update only the top-level version and packages[""].version; never replace version substrings across dependency entries.
2. Run `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, and the extension's `npm ci`, `npm run check`, and `npm test`.
3. Run `./scripts/package.sh`, then `python3 tests/install_test.py` on Apple Silicon. The installer test compares the binary version with Cargo.toml.
4. Commit all source changes and push main. Create and push the matching version tag, for example `v0.1.0`.
5. Confirm the repository is public with `gh repo view altonwells/x-bot-follower-remover --json visibility`.
6. Create the release from the pushed tag:

```sh
gh release create v0.1.0 --repo altonwells/x-bot-follower-remover --verify-tag \
  --title 'remover v0.1.0' --notes-file docs/RELEASE_NOTES.md \
  dist/remover-macos-arm64.zip dist/remover-extension.zip dist/SHA256SUMS
```

Never replace assets on an existing release silently. Publish a new version for binary changes. Source-only installer fixes may be made on main; existing releases include their original installer for local install/uninstall. Installed users update by rerunning the public install command, then reloading the unpacked Chrome extension.

The installer deliberately does not install a Chrome extension through browser policy or modify the user's account. A user must load the extension. Pairing then runs through Chrome native messaging while the TUI is open. Moving from an earlier manually unpacked path may change its extension ID; re-pair with `remover pair --reset` if needed.
