# Rename report

Project: `x-bot-follower-remover`. Command: `remover`. Chrome: **Remover**. Version: **0.3.0**.

Git history is preserved. No tracked filenames or folders contained the old name, so no tracked path required `git mv`. The checkout directory is unchanged. Release archives and generated executables use the new command name.

## Changed and added files

- `docs/RENAME_REPORT.md`
- `.github/repo-meta.md`
- `CHANGELOG.md`
- `Cargo.lock`
- `Cargo.toml`
- `README.md`
- `docs/DEVELOPMENT.md`
- `docs/GUIDE.md`
- `docs/RELEASE_NOTES.md`
- `docs/RELEASING.md`
- `docs/VERIFICATION.md`
- `docs/options-preview.png`
- `docs/previews/account-error.png`
- `docs/previews/cleanse.png`
- `docs/previews/dashboard.png`
- `docs/previews/pairing-minimum.png`
- `docs/previews/pairing.png`
- `docs/previews/simple-start.png`
- `docs/previews/simple-worker.png`
- `examples/preview.rs`
- `extension/build.mjs`
- `extension/icons/r-128.png`
- `extension/icons/r-16.png`
- `extension/icons/r-32.png`
- `extension/icons/r-48.png`
- `extension/manifest.json`
- `extension/options.css`
- `extension/options.html`
- `extension/package-lock.json`
- `extension/package.json`
- `extension/src/background.ts`
- `extension/src/options.ts`
- `extension/src/pairing.ts`
- `extension/src/parsers.ts`
- `extension/tests/options.test.ts`
- `extension/tests/pairing.test.ts`
- `install.sh`
- `protocol/schema.json`
- `references/README.md`
- `scripts/package.py`
- `scripts/render-icons.swift`
- `scripts/render-preview.swift`
- `src/app.rs`
- `src/bridge.rs`
- `src/config.rs`
- `src/main.rs`
- `src/native.rs`
- `src/setup.rs`
- `src/ui.rs`
- `tests/background.rs`
- `tests/background_cli_test.py`
- `tests/bridge.rs`
- `tests/cleanup.rs`
- `tests/config_rename.rs`
- `tests/controller.rs`
- `tests/install_test.py`
- `tests/native_host.rs`
- `tests/policy_contract.rs`
- `tests/presentation.rs`
- `tests/setup.rs`
- `tests/terminal_test.py`

## Retained old names

- `src/config.rs` and `tests/config_rename.rs`: locate and verify the legacy data folder without moving a running worker’s database or process lock.
- `install.sh` and `tests/install_test.py`: support old environment overrides and ownership markers, recognize the existing PATH entry, and retire only the owned old command during an in-place upgrade.
- `README.md`: tell existing users how to stop the old worker and preserve their original data folder.
- `docs/VERIFICATION.md`: preserve historical command and behavior records.
- `.git` history and current remote: historical data is not rewritten. The GitHub rename command is supplied for manual execution as requested.
- Ignored old build outputs and old installed files: retained rather than removed speculatively. Current release archives contain the renamed products only.

## GitHub metadata

The rename, description, and topic commands are in [repo-meta.md](../.github/repo-meta.md). They are provided for manual execution, not run by the installer. Repository visibility remains private.

## Validation

151 tests passed across Rust, TypeScript, installer, terminal, and background-process suites. Clippy, TypeScript checking, release packaging, icon dimensions, and dependency-lock comparisons passed. See [verification](VERIFICATION.md).
