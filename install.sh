#!/usr/bin/env bash
# Install a checksum-verified DCTX0 release. No sudo or shell-profile changes.
# Keeping execution inside one parsed block prevents a truncated download from running main.
{
set -euo pipefail

fail() { printf 'dctx0: %s\n' "$*" >&2; exit 1; }
usage() {
    cat <<'HELP'
Install DCTX0 from GitHub Releases.

Usage: bash install.sh [--version VERSION] [--bin-dir DIRECTORY]

  --version VERSION     A release tag, such as v0.1.0 (default: latest stable)
  --bin-dir DIRECTORY   Destination (default: $HOME/.local/bin)
  --help                Show this help

DCTX0_REPOSITORY overrides Haardt/dctx0ctl for a fork.
DCTX0_INSTALL_DIR overrides the default destination.
Supports Linux x86_64/ARM64 (glibc 2.35+) and macOS Intel/Apple Silicon (14+).
HELP
}
valid_version() {
    [[ "$1" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$ ]]
}
cleanup() {
    if [[ -n "${staged_binary:-}" ]]; then rm -f -- "$staged_binary"; fi
    if [[ -n "${download_dir:-}" ]]; then rm -rf -- "$download_dir"; fi
}
fetch() {
    curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' \
        --tlsv1.2 --connect-timeout 15 --max-time 180 --retry 3 "$@"
}
main() {
    local version=latest
    local bin_dir="${DCTX0_INSTALL_DIR:-${HOME:?HOME must be set}/.local/bin}"
    local repository="${DCTX0_REPOSITORY:-Haardt/dctx0ctl}"
    local release_base="https://github.com/$repository/releases"
    local system architecture target resolved_url archive digest actual
    local glibc glibc_major glibc_minor installed_version
    download_dir=''
    staged_binary=''
    trap cleanup EXIT
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --version)
                [[ $# -ge 2 && -n "$2" ]] || fail '--version requires a release version'
                version="$2"; shift 2 ;;
            --bin-dir)
                [[ $# -ge 2 && -n "$2" ]] || fail '--bin-dir requires a directory'
                bin_dir="$2"; shift 2 ;;
            --help|-h) usage; return ;;
            *) fail "Unknown argument: $1 (use --help)" ;;
        esac
    done
    [[ "$repository" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || fail 'Invalid DCTX0_REPOSITORY'
    if [[ "$version" != latest ]]; then
        [[ "$version" == v* ]] || version="v$version"
        valid_version "$version" || fail 'Version must be a release tag such as v0.1.0'
    fi
    for tool in curl uname mktemp tar awk cp chmod mv mkdir; do
        command -v "$tool" >/dev/null 2>&1 || fail "Required command not found: $tool"
    done
    system="$(uname -s)"
    architecture="$(uname -m)"
    case "$system/$architecture" in
        Linux/x86_64|Linux/amd64) target=x86_64-unknown-linux-gnu ;;
        Linux/aarch64|Linux/arm64) target=aarch64-unknown-linux-gnu ;;
        Darwin/x86_64) target=x86_64-apple-darwin ;;
        Darwin/arm64|Darwin/aarch64) target=aarch64-apple-darwin ;;
        *) fail "Unsupported platform: $system/$architecture" ;;
    esac
    if [[ "$system" == Linux ]]; then
        command -v getconf >/dev/null 2>&1 || fail 'Required command not found: getconf'
        glibc="$(getconf GNU_LIBC_VERSION 2>/dev/null)" || fail 'Linux releases require glibc 2.35 or newer'
        [[ "$glibc" =~ ^glibc\ ([0-9]+)\.([0-9]+)$ ]] || fail 'Cannot determine glibc version'
        glibc_major="${BASH_REMATCH[1]}"
        glibc_minor="${BASH_REMATCH[2]}"
        (( glibc_major > 2 || (glibc_major == 2 && glibc_minor >= 35) )) || fail "glibc 2.35 or newer is required (found $glibc)"
    else
        command -v sw_vers >/dev/null 2>&1 || fail 'Required command not found: sw_vers'
        local mac_version mac_major
        mac_version="$(sw_vers -productVersion)"
        mac_major="${mac_version%%.*}"
        [[ "$mac_major" =~ ^[0-9]+$ ]] && (( mac_major >= 14 )) || fail 'macOS 14 or newer is required'
    fi
    if command -v sha256sum >/dev/null 2>&1; then
        checksum() { sha256sum "$1"; }
    elif command -v shasum >/dev/null 2>&1; then
        checksum() { shasum -a 256 "$1"; }
    else
        fail 'Required command not found: sha256sum or shasum'
    fi
    [[ ! -L "$bin_dir/dctx0" ]] || fail 'Destination dctx0 is a symlink; choose another --bin-dir'
    [[ ! -e "$bin_dir/dctx0" || -f "$bin_dir/dctx0" ]] || fail 'Destination dctx0 is not a regular file'
    if [[ "$version" == latest ]]; then
        resolved_url="$(fetch --output /dev/null --write-out '%{url_effective}' "$release_base/latest")" \
            || fail 'Cannot resolve the latest published release; select an existing --version'
        [[ "$resolved_url" == "$release_base/tag/"* ]] || fail 'Unexpected latest-release redirect'
        version="${resolved_url##*/}"
        valid_version "$version" || fail 'The latest release has an invalid version tag'
    fi
    archive="dctx0-$version-$target.tar.gz"
    download_dir="$(mktemp -d)"
    printf 'Downloading DCTX0 %s for %s…\n' "$version" "$target"
    fetch --output "$download_dir/SHA256SUMS" "$release_base/download/$version/SHA256SUMS" \
        || fail "Cannot download checksums for $version"
    digest="$(awk -v asset="$archive" '$2 == asset && NF == 2 { count++; hash=$1 } END { if (count != 1) exit 1; print hash }' "$download_dir/SHA256SUMS")" \
        || fail "No unique checksum for $archive"
    [[ "$digest" =~ ^[0-9a-f]{64}$ ]] || fail "Invalid checksum for $archive"
    fetch --output "$download_dir/$archive" "$release_base/download/$version/$archive" \
        || fail "Cannot download $archive"
    actual="$(checksum "$download_dir/$archive")"
    actual="${actual%% *}"
    [[ "$actual" == "$digest" ]] || fail "Checksum mismatch for $archive; installation unchanged"
    # Stream only the expected member. Never unpack archive paths or symlinks.
    tar -xOzf "$download_dir/$archive" -- dctx0 > "$download_dir/dctx0" \
        || fail 'Release archive has no readable dctx0 executable'
    mkdir -p -- "$bin_dir"
    staged_binary="$(mktemp "$bin_dir/.dctx0-install.XXXXXX")"
    cp -- "$download_dir/dctx0" "$staged_binary"
    chmod 755 "$staged_binary"
    installed_version="$("$staged_binary" --version)" || fail 'Downloaded binary could not run; installation unchanged'
    [[ "$installed_version" == "dctx0 ${version#v}" ]] || fail 'Downloaded binary version does not match the release tag'
    mv -f -- "$staged_binary" "$bin_dir/dctx0"
    staged_binary=''
    printf 'Installed %s → %s/dctx0\n' "$installed_version" "$bin_dir"
    case ":${PATH:-}:" in
        *":$bin_dir:"*) ;;
        *) printf 'Add the install directory to your PATH:\n  export PATH=%q:$PATH\n' "$bin_dir" ;;
    esac
}
main "$@"
}
