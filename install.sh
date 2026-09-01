#!/usr/bin/env bash
# Install a Harness-Gate binary from one immutable GitHub release tag.

set -Eeuo pipefail
umask 022

REPO="musutrade/Harness-Gate"
BINARY_NAME="harness-gate"
RELEASE_BASE_URL="https://github.com/${REPO}/releases/download"
VERSION="${HARNESS_GATE_VERSION:-}"
INSTALL_DIR="${INSTALL_DIR:-${HOME}/.local/bin}"
OS=""
ARCH=""
PLATFORM=""
INSTALL_NAME="$BINARY_NAME"
ATOMIC_TEMPORARY=""

usage() {
    cat <<'EOF'
Usage: install.sh --version vX.Y.Z [--install-dir DIR]
       install.sh --version vX.Y.Z --from-source [--install-dir DIR]

The version is required so the download is bound to an immutable release tag.
The installer verifies SHA256 and the Sigstore keyless certificate before it
changes the destination directory.
EOF
}

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

cleanup_atomic_temporary() {
    if [[ -n "${ATOMIC_TEMPORARY:-}" ]]; then
        rm -f -- "$ATOMIC_TEMPORARY" 2>/dev/null || true
        ATOMIC_TEMPORARY=""
    fi
}

abort_on_signal() {
    local status="$1"
    cleanup_atomic_temporary
    exit "$status"
}

validate_version() {
    [[ "$VERSION" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$ ]] \
        || die "version must be an exact v-prefixed SemVer (for example v0.3.3)"
    if [[ "$VERSION" == *-* ]]; then
        local prerelease="${VERSION#*-}"
        prerelease="${prerelease%%+*}"
        local identifier
        IFS='.' read -r -a identifiers <<<"$prerelease"
        for identifier in "${identifiers[@]}"; do
            [[ ! "$identifier" =~ ^0[0-9]+$ ]] \
                || die "numeric prerelease identifiers must not contain leading zeroes"
        done
    fi
}

detect_platform() {
    local os
    local arch
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"

    case "$os" in
        linux*) OS="linux" ;;
        darwin*) OS="macos" ;;
        mingw*|msys*|cygwin*) OS="windows" ;;
        *) die "unsupported operating system: $os" ;;
    esac

    case "$arch" in
        x86_64|amd64) ARCH="amd64" ;;
        arm64|aarch64) ARCH="arm64" ;;
        *) die "unsupported architecture: $arch" ;;
    esac

    if [[ "$OS" == windows && "$ARCH" == arm64 ]]; then
        die "no Windows arm64 release asset is published"
    fi
    PLATFORM="${OS}-${ARCH}"
    INSTALL_NAME="$BINARY_NAME"
    if [[ "$OS" == windows ]]; then
        INSTALL_NAME="${BINARY_NAME}.exe"
    fi
}

download() {
    local url="$1"
    local output="$2"
    if command -v curl >/dev/null 2>&1; then
        curl --fail --show-error --location --proto '=https' --tlsv1.2 \
            --retry 3 --retry-all-errors --output "$output" "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget --https-only --tries=3 --output-document="$output" "$url"
    else
        die "curl or wget is required"
    fi
    [[ ! -L "$output" && -f "$output" && -s "$output" ]] \
        || die "downloaded file is missing, symlinked, or empty: $url"
}

verify_checksum() {
    local dist="$1"
    local filename="$2"
    local manifest="$dist/SHA256SUMS"
    local selected="$dist/checksum.selected"
    local matches

    matches="$(LC_ALL=C awk -v expected="$filename" '$2 == expected { print; count++ } END { if (count != 1) exit 1 }' "$manifest")" \
        || die "SHA256SUMS does not contain exactly one entry for $filename"
    printf '%s\n' "$matches" >"$selected"
    if command -v sha256sum >/dev/null 2>&1; then
        (cd "$dist" && sha256sum --check --status "$(basename "$selected")") \
            || die "SHA256 checksum verification failed for $filename"
    elif command -v shasum >/dev/null 2>&1; then
        (cd "$dist" && shasum -a 256 -c "$(basename "$selected")") \
            || die "SHA256 checksum verification failed for $filename"
    else
        die "sha256sum or shasum is required"
    fi
}

verify_signature() {
    local dist="$1"
    local filename="$2"
    command -v cosign >/dev/null 2>&1 \
        || die "cosign is required for Sigstore verification (see https://docs.sigstore.dev/cosign/system_config/installation/)"

    # The release workflow's OIDC identity is bound to this exact immutable tag.
    local escaped_version
    escaped_version="${VERSION//./\\.}"
    escaped_version="${escaped_version//+/\\+}"
    cosign verify-blob \
        --signature "$dist/${filename}.sig" \
        --certificate "$dist/${filename}.crt" \
        --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
        --certificate-identity-regexp "^https://github.com/${REPO}/.github/workflows/release\\.yml@refs/tags/${escaped_version}$" \
        "$dist/$filename" \
        || die "Sigstore verification failed for $filename"
}

validate_install_dir() {
    [[ "$INSTALL_DIR" = /* ]] || die "install directory must be an absolute path: $INSTALL_DIR"
    [[ "$INSTALL_DIR" != / ]] || die "install directory must not be the filesystem root"
    [[ "$INSTALL_DIR" != *$'\n'* && "$INSTALL_DIR" != *$'\r'* ]] \
        || die "install directory contains a line break"
    local component
    local current="/"
    local relative="${INSTALL_DIR#/}"
    IFS='/' read -r -a components <<<"$relative"
    for component in "${components[@]}"; do
        [[ -n "$component" && "$component" != "." && "$component" != ".." ]] \
            || die "install directory must not contain empty, dot, or traversal components"
        current="${current%/}/$component"
        [[ ! -L "$current" ]] || die "install directory contains a symlink component: $current"
    done
    if [[ -e "$INSTALL_DIR" && ! -d "$INSTALL_DIR" ]]; then
        die "install directory is not a directory: $INSTALL_DIR"
    fi
    if [[ ! -e "$INSTALL_DIR" ]]; then
        mkdir -p "$INSTALL_DIR"
        chmod 0755 "$INSTALL_DIR"
    fi
    [[ ! -L "$INSTALL_DIR" ]] || die "install directory must not be a symlink: $INSTALL_DIR"
    [[ -d "$INSTALL_DIR" && -w "$INSTALL_DIR" ]] || die "install directory is not writable: $INSTALL_DIR"

    # Do not install into a directory writable by group or other users.
    local mode
    if mode="$(stat -c '%a' "$INSTALL_DIR" 2>/dev/null)"; then
        :
    else
        mode="$(stat -f '%Lp' "$INSTALL_DIR" 2>/dev/null)" \
            || die "cannot inspect install directory permissions: $INSTALL_DIR"
    fi
    [[ "$mode" =~ ^[0-7]+$ ]] || die "cannot inspect install directory permissions: $INSTALL_DIR"
    local mode_number=$((8#$mode))
    (( (mode_number & 8#022) == 0 )) || die "install directory is group/other writable: $INSTALL_DIR"

    local target="$INSTALL_DIR/$INSTALL_NAME"
    [[ ! -L "$target" ]] || die "refusing to replace symlink target: $target"
    if [[ -e "$target" && ! -f "$target" ]]; then
        die "existing install target is not a regular file: $target"
    fi
}

atomic_install() {
    local source="$1"
    [[ ! -L "$source" && -f "$source" ]] || die "installation source is not a regular file: $source"
    validate_install_dir
    local target="$INSTALL_DIR/$INSTALL_NAME"
    ATOMIC_TEMPORARY="$(mktemp "$INSTALL_DIR/.${INSTALL_NAME}.XXXXXX")" \
        || die "cannot allocate an atomic install file in $INSTALL_DIR"
    if ! cp "$source" "$ATOMIC_TEMPORARY"; then
        cleanup_atomic_temporary
        die "cannot stage binary in $INSTALL_DIR"
    fi
    if ! chmod 0755 "$ATOMIC_TEMPORARY"; then
        cleanup_atomic_temporary
        die "cannot set executable mode in $INSTALL_DIR"
    fi
    if ! mv -f -- "$ATOMIC_TEMPORARY" "$target"; then
        cleanup_atomic_temporary
        die "cannot atomically install $target"
    fi
    ATOMIC_TEMPORARY=""
    printf 'installed %s %s (SHA256 %s)\n' "$BINARY_NAME" "$VERSION" "$(file_sha256 "$target")"
}

file_sha256() {
    local path="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$path" | awk '{print $1}'
    else
        shasum -a 256 "$path" | awk '{print $1}'
    fi
}

install_binary() {
    local temporary_root="$1"
    detect_platform
    local extension=""
    [[ "$OS" == windows ]] && extension=".exe"
    local filename="${BINARY_NAME}-${PLATFORM}${extension}"
    local base_url="${RELEASE_BASE_URL}/${VERSION}"

    printf 'downloading and verifying %s from immutable tag %s\n' "$filename" "$VERSION"
    download "${base_url}/${filename}" "$temporary_root/$filename"
    download "${base_url}/SHA256SUMS" "$temporary_root/SHA256SUMS"
    download "${base_url}/SHA256SUMS.sig" "$temporary_root/SHA256SUMS.sig"
    download "${base_url}/SHA256SUMS.crt" "$temporary_root/SHA256SUMS.crt"
    download "${base_url}/${filename}.sig" "$temporary_root/${filename}.sig"
    download "${base_url}/${filename}.crt" "$temporary_root/${filename}.crt"
    verify_signature "$temporary_root" "SHA256SUMS"
    verify_checksum "$temporary_root" "$filename"
    verify_signature "$temporary_root" "$filename"
    atomic_install "$temporary_root/$filename"
}

install_from_source() {
    command -v cargo >/dev/null 2>&1 || die "Rust cargo is required for source installation"
    command -v git >/dev/null 2>&1 || die "git is required for source installation"
    detect_platform
    local source_root="$1/source"
    git clone --depth 1 --no-checkout --single-branch --no-tags \
        "https://github.com/${REPO}.git" "$source_root"
    git -C "$source_root" fetch --depth 1 origin \
        "refs/tags/$VERSION:refs/tags/$VERSION"
    git -C "$source_root" checkout --detach "refs/tags/$VERSION"
    local tag_commit
    local head_commit
    tag_commit="$(git -C "$source_root" rev-parse --verify "refs/tags/$VERSION^{commit}")" \
        || die "source tag is unavailable: $VERSION"
    head_commit="$(git -C "$source_root" rev-parse --verify HEAD)" \
        || die "source checkout has no commit"
    [[ "$tag_commit" == "$head_commit" ]] \
        || die "source checkout does not resolve the requested immutable tag: $VERSION"
    [[ -z "$(git -C "$source_root" status --porcelain)" ]] \
        || die "source checkout is unexpectedly modified"
    local cargo_root="$1/cargo-root"
    cargo install --locked --path "$source_root/tools/harness-gate" --root "$cargo_root"
    local built_binary="$cargo_root/bin/$BINARY_NAME"
    if [[ ! -f "$built_binary" && -f "${built_binary}.exe" ]]; then
        built_binary="${built_binary}.exe"
    fi
    [[ -f "$built_binary" ]] || die "cargo did not produce $BINARY_NAME"
    atomic_install "$built_binary"
    printf 'installed %s %s from immutable source tag\n' "$BINARY_NAME" "$VERSION"
}

main() {
    local from_source=0
    while (($# > 0)); do
        case "$1" in
            --version)
                (($# >= 2)) || die "--version requires a value"
                VERSION="$2"
                shift 2
                ;;
            --install-dir)
                (($# >= 2)) || die "--install-dir requires a value"
                INSTALL_DIR="$2"
                shift 2
                ;;
            --from-source)
                from_source=1
                shift
                ;;
            -h|--help)
                usage
                return 0
                ;;
            *)
                usage >&2
                die "unknown argument: $1"
                ;;
        esac
    done

    [[ -n "$VERSION" ]] || { usage >&2; die "--version is required"; }
    validate_version
    local temporary_root
    temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/harness-gate-install.XXXXXXXX")" \
        || die "cannot create temporary installation directory"
    trap 'cleanup_atomic_temporary; rm -rf "${temporary_root:-}"' EXIT
    trap 'abort_on_signal 129' HUP
    trap 'abort_on_signal 130' INT
    trap 'abort_on_signal 143' TERM
    if ((from_source)); then
        install_from_source "$temporary_root"
    else
        install_binary "$temporary_root"
    fi
}

main "$@"
