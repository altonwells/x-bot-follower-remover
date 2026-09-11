#!/bin/sh
# Private-release installer. No sudo, build tools, or X credentials required.
set -eu

fail() { printf 'forgive-me: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "Required command missing: $1"; }

main() {
    repo=altonwells/forgive-me
    install_dir=${FORGIVE_ME_INSTALL_DIR:-"$HOME/.local/share/forgive-me"}
    bin_dir=${FORGIVE_ME_BIN_DIR:-"$HOME/.local/bin"}
    from_dir= version= uninstall=0 modify_path=1
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --from) [ "$#" -ge 2 ] || fail '--from needs a release directory'; from_dir=$2; shift 2 ;;
            --version) [ "$#" -ge 2 ] || fail '--version needs a tag'; version=$2; shift 2 ;;
            --no-modify-path) modify_path=0; shift ;;
            --uninstall) uninstall=1; shift ;;
            --help) printf '%s\n' 'Usage: sh install.sh [--version v0.1.0] [--from ./dist] [--no-modify-path] [--uninstall]' 'Overrides: FORGIVE_ME_INSTALL_DIR, FORGIVE_ME_BIN_DIR'; return ;;
            *) fail "Unknown option: $1" ;;
        esac
    done
    case "$install_dir" in /*) ;; *) fail 'Install directory must be absolute' ;; esac
    case "$bin_dir" in /*) ;; *) fail 'Binary directory must be absolute' ;; esac
    [ "$install_dir" != / ] && [ "$install_dir" != "$HOME" ] || fail 'Choose a dedicated installation directory'
    target=$install_dir/bundle/forgive-me
    marker=$install_dir/.forgive-me-install
    path_file=
    case "${SHELL:-}" in
        */zsh) path_file=${ZDOTDIR:-"$HOME"}/.zshrc ;;
        */bash)
            path_file=$HOME/.bash_profile
            for candidate in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile"; do
                if [ -f "$candidate" ]; then path_file=$candidate; break; fi
            done ;;

    esac
    if [ "$uninstall" -eq 1 ]; then
        [ -f "$marker" ] || fail "No managed installation at $install_dir"
        mkdir "$install_dir/.install-lock" 2>/dev/null || fail 'Another installer is running'
        trap 'rmdir "$install_dir/.install-lock" 2>/dev/null || :' EXIT
        if [ -L "$bin_dir/forgive-me" ] && [ "$(readlink "$bin_dir/forgive-me")" = "$target" ]; then
            rm "$bin_dir/forgive-me"
        fi
        if [ -x "$target" ]; then "$target" unregister-host >/dev/null 2>&1 || :; fi
        rm -rf "$install_dir"
        printf '%s\n' 'Uninstalled forgive-me. Your cleanup database and Chrome extension settings were preserved.' 'The shared ~/.local/bin PATH entry is retained. Remove the extension from chrome://extensions if desired.'
        return
    fi
    case "$(uname -s)/$(uname -m)" in
        Darwin/arm64) asset=forgive-me-macos-arm64.zip ;;
        *) fail 'This release provides an Apple Silicon macOS binary. Other platforms must build from source; no incompatible binary was installed.' ;;
    esac
    need unzip
    if command -v shasum >/dev/null 2>&1; then hash_tool=shasum; else need sha256sum; hash_tool=sha256sum; fi
    if [ -e "$install_dir" ] && [ ! -f "$marker" ]; then
        fail "Refusing to replace an unmanaged directory: $install_dir"
    fi
    if [ -e "$bin_dir/forgive-me" ] || [ -L "$bin_dir/forgive-me" ]; then
        [ -L "$bin_dir/forgive-me" ] && [ "$(readlink "$bin_dir/forgive-me")" = "$target" ] || fail "An unmanaged executable already exists at $bin_dir/forgive-me"
    fi
    download_dir=$(mktemp -d "${TMPDIR:-/tmp}/forgive-me-download.XXXXXX")
    stage_dir= link_tmp= locked=0
    cleanup() {
        if [ "$locked" -eq 1 ]; then
            # Restore the preceding version if replacement was interrupted before commit.
            if [ -d "$install_dir/.previous" ] && [ ! -d "$install_dir/bundle" ]; then mv "$install_dir/.previous" "$install_dir/bundle"; fi
            rmdir "$install_dir/.install-lock" 2>/dev/null || :
        fi
        [ -z "$stage_dir" ] || rm -rf "$stage_dir"
        [ -z "$link_tmp" ] || rm -f "$link_tmp"
        rm -rf "$download_dir"
    }
    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM HUP
    if [ -n "$from_dir" ]; then
        cp "$from_dir/$asset" "$download_dir/$asset"
        cp "$from_dir/SHA256SUMS" "$download_dir/SHA256SUMS"
    else
        need gh
        gh auth status --hostname github.com >/dev/null 2>&1 || fail 'Run gh auth login first; this is a private GitHub release.'
        if [ -z "$version" ]; then version=$(gh release view --repo "$repo" --json tagName --jq .tagName); fi
        case "$version" in v[0-9]*) ;; *) fail 'Expected a version tag such as v0.1.0' ;; esac
        printf 'Downloading %s from %s…\n' "$version" "$repo"
        gh release download "$version" --repo "$repo" --pattern "$asset" --pattern SHA256SUMS --dir "$download_dir"
    fi
    expected=$(awk -v file="$asset" '$2 == file {print $1}' "$download_dir/SHA256SUMS")
    [ "${#expected}" -eq 64 ] || fail 'Missing or ambiguous release checksum'
    case "$expected" in *[!0-9a-fA-F]*) fail 'Invalid release checksum' ;; esac
    if [ "$hash_tool" = shasum ]; then actual=$(shasum -a 256 "$download_dir/$asset" | awk '{print $1}');
    else actual=$(sha256sum "$download_dir/$asset" | awk '{print $1}'); fi
    [ "$actual" = "$expected" ] || fail 'Checksum mismatch; existing installation was not changed'
    unzip -Z -1 "$download_dir/$asset" > "$download_dir/entries"
    awk 'index($0,"forgive-me/") != 1 || /(^|\/)\.\.(\/|$)/ || /\\/ {bad=1} END {exit bad}' "$download_dir/entries" || fail 'Unsafe archive paths'
    unzip -Z -l "$download_dir/$asset" > "$download_dir/details"
    if grep '^l' "$download_dir/details" >/dev/null; then fail 'Archive contains unexpected symbolic links'; fi
    unzip -q "$download_dir/$asset" -d "$download_dir/unpacked"
    payload=$download_dir/unpacked/forgive-me
    [ -f "$payload/forgive-me" ] && [ -f "$payload/forgive-me-extension/manifest.json" ] && [ -f "$payload/install.sh" ] || fail 'Incomplete release bundle'
    chmod 755 "$payload/forgive-me"
    installed_version=$("$payload/forgive-me" --version)
    case "$installed_version" in 'forgive-me '*) ;; *) fail 'Release executable failed validation' ;; esac
    if [ -n "$version" ]; then [ "$installed_version" = "forgive-me ${version#v}" ] || fail 'Release tag and executable version disagree'; fi
    mkdir -p "$install_dir" "$bin_dir"
    chmod 700 "$install_dir"
    : > "$marker"
    mkdir "$install_dir/.install-lock" 2>/dev/null || fail 'Another installer is running; retry after it finishes'
    locked=1
    stage_dir=$(mktemp -d "$install_dir/.stage.XXXXXX")
    cp -R "$payload" "$stage_dir/bundle"
    # Both renames stay on the same filesystem. The public extension path remains stable.
    if [ -d "$install_dir/.previous" ] && [ ! -d "$install_dir/bundle" ]; then mv "$install_dir/.previous" "$install_dir/bundle"; fi
    rm -rf "$install_dir/.previous"
    if [ -d "$install_dir/bundle" ]; then mv "$install_dir/bundle" "$install_dir/.previous"; fi
    mv "$stage_dir/bundle" "$install_dir/bundle"
    link_tmp=$bin_dir/.forgive-me-link.$$
    ln -s "$target" "$link_tmp"
    mv -f "$link_tmp" "$bin_dir/forgive-me"
    link_tmp=
    rm -rf "$install_dir/.previous"
    if [ "$modify_path" -eq 1 ] && [ "$bin_dir" = "$HOME/.local/bin" ] && [ -n "$path_file" ]; then
        mkdir -p "$(dirname "$path_file")"
        if ! grep -F '# >>> forgive-me PATH >>>' "$path_file" >/dev/null 2>&1; then
            cat >> "$path_file" <<'PATH_BLOCK'

# >>> forgive-me PATH >>>
case ":$PATH:" in
    *":$HOME/.local/bin:"*) ;;
    *) export PATH="$HOME/.local/bin:$PATH" ;;
esac
# <<< forgive-me PATH <<<
PATH_BLOCK
        fi
    fi
    printf '\nInstalled %s\nCommand: %s/forgive-me\nChrome extension: %s/bundle/forgive-me-extension\n\n' "$installed_version" "$bin_dir" "$install_dir"
    case ":$PATH:" in *":$bin_dir:"*) ;; *) printf '%s\n' 'Open a new terminal, or add the binary directory to PATH in this terminal.' ;; esac
    printf '%s\n' 'Next: run forgive-me, then press Enter to start Chrome setup. Use forgive-me --demo to preview.' 'Chrome requires Load unpacked once. Pairing then runs automatically.' 'For updates, reopen the TUI, Reload the extension, then select Connect automatically.'
}

main "$@"
