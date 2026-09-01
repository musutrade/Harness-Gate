#!/usr/bin/env bash
# Offline contract tests for the immutable installer boundary.

set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/harness-gate-installer-test.XXXXXXXX")"
trap 'rm -rf "$TEMP_ROOT"' EXIT
FIXTURE="$TEMP_ROOT/fixture"
FAKE_BIN="$TEMP_ROOT/fake-bin"
mkdir -p "$FIXTURE" "$FAKE_BIN"

binary_name="harness-gate-linux-amd64"
printf 'verified fixture binary\n' >"$FIXTURE/$binary_name"
windows_binary_name="harness-gate-windows-amd64.exe"
printf 'verified windows fixture binary\n' >"$FIXTURE/$windows_binary_name"

hash_file() {
    local path="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$path" | awk '{print $1}'
    else
        shasum -a 256 "$path" | awk '{print $1}'
    fi
}

: >"$FIXTURE/SHA256SUMS"
printf '%s  %s\n' "$(hash_file "$FIXTURE/$binary_name")" "$binary_name" >>"$FIXTURE/SHA256SUMS"
printf '%s  %s\n' "$(hash_file "$FIXTURE/$windows_binary_name")" "$windows_binary_name" >>"$FIXTURE/SHA256SUMS"
printf 'signed-subject=SHA256SUMS\n' >"$FIXTURE/SHA256SUMS.sig"
printf 'identity=https://github.com/musutrade/Harness-Gate/.github/workflows/release.yml@refs/tags/v0.3.3\n' >"$FIXTURE/SHA256SUMS.crt"
printf 'signed-subject=%s\n' "$binary_name" >"$FIXTURE/$binary_name.sig"
printf 'identity=https://github.com/musutrade/Harness-Gate/.github/workflows/release.yml@refs/tags/v0.3.3\n' >"$FIXTURE/$binary_name.crt"
printf 'signed-subject=%s\n' "$windows_binary_name" >"$FIXTURE/$windows_binary_name.sig"
printf 'identity=https://github.com/musutrade/Harness-Gate/.github/workflows/release.yml@refs/tags/v0.3.3\n' >"$FIXTURE/$windows_binary_name.crt"

cat >"$FAKE_BIN/curl" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
expected=(--fail --show-error --location --proto '=https' --tlsv1.2 --retry 3 --retry-all-errors --output)
[[ "$#" -eq 12 ]] || { printf 'unexpected curl argv: %s\n' "$*" >&2; exit 2; }
for argument in "${expected[@]}"; do
    [[ "${1:-}" == "$argument" ]] || { printf 'unexpected curl option: %s\n' "${1:-}" >&2; exit 2; }
    shift
done
output="$1"
url="$2"
expected_prefix="https://github.com/musutrade/Harness-Gate/releases/download/v0.3.3/"
[[ "$url" == "$expected_prefix"* && "$url" != *'?'* && "$url" != *'#'* ]] || exit 22
filename="${url##*/}"
[[ -n "$filename" && "$filename" != */* ]] || exit 22
source_file="$HARNESS_GATE_TEST_FIXTURE/$filename"
[[ -f "$source_file" ]] || exit 22
/bin/cp "$source_file" "$output"
EOF
chmod 755 "$FAKE_BIN/curl"

cat >"$FAKE_BIN/cosign" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
original_args="$*"
[[ "${1:-}" == verify-blob ]] || exit 2
shift
signature=""
certificate=""
issuer=""
identity=""
subject=""
while (($# > 0)); do
    case "$1" in
        --signature) signature="${2:-}"; shift 2 ;;
        --certificate) certificate="${2:-}"; shift 2 ;;
        --certificate-oidc-issuer) issuer="${2:-}"; shift 2 ;;
        --certificate-identity-regexp) identity="${2:-}"; shift 2 ;;
        --*) exit 2 ;;
        *) [[ -z "$subject" ]] || exit 2; subject="$1"; shift ;;
    esac
done
expected_identity='^https://github.com/musutrade/Harness-Gate/.github/workflows/release\.yml@refs/tags/v0\.3\.3$'
[[ "$issuer" == 'https://token.actions.githubusercontent.com' ]] || exit 1
[[ "$identity" == "$expected_identity" ]] || exit 1
[[ -n "$signature" && -n "$certificate" && -n "$subject" ]] || exit 1
[[ "$(basename "$signature")" == "$(basename "$subject").sig" ]] || exit 1
[[ "$(basename "$certificate")" == "$(basename "$subject").crt" ]] || exit 1
grep -Fxq 'identity=https://github.com/musutrade/Harness-Gate/.github/workflows/release.yml@refs/tags/v0.3.3' "$certificate" || exit 1
grep -Fxq "signed-subject=$(basename "$subject")" "$signature" || exit 1
printf '%s\n' "$original_args" >>"$HARNESS_GATE_TEST_COSIGN_LOG"
if [[ "${HARNESS_GATE_TEST_COSIGN_FAIL:-0}" == 1 ]]; then exit 1; fi
exit 0
EOF
chmod 755 "$FAKE_BIN/cosign"

cat >"$FAKE_BIN/git" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
if [[ "$1" == clone ]]; then
    [[ "$2" == --depth && "$3" == 1 && "$4" == --no-checkout && "$5" == --single-branch && "$6" == --no-tags ]] || exit 2
    [[ "$7" == https://github.com/musutrade/Harness-Gate.git ]] || exit 2
    destination="$8"
    mkdir -p "$destination"
    exit 0
fi
if [[ "$1" == -C ]]; then
    source_root="$2"
    case "$3" in
        fetch)
            [[ "$4" == --depth && "$5" == 1 && "$6" == origin && "$7" == refs/tags/v0.3.3:refs/tags/v0.3.3 ]] || exit 2
            ;;
        checkout)
            [[ "$4" == --detach && "$5" == refs/tags/v0.3.3 ]] || exit 2
            mkdir -p "$source_root/tools/harness-gate"
            printf 'fixture-commit\n' >"$source_root/.fake-head"
            ;;
        rev-parse)
            [[ "$4" == --verify ]] || exit 2
            case "$5" in
                'refs/tags/v0.3.3^{commit}'|HEAD) printf 'fixture-commit\n' ;;
                *) exit 2 ;;
            esac
            ;;
        status)
            [[ "$4" == --porcelain ]] || exit 2
            ;;
        *) exit 2 ;;
    esac
    exit 0
fi
exit 2
EOF
chmod 755 "$FAKE_BIN/git"

cat >"$FAKE_BIN/cargo" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
[[ "${1:-}" == install && "${2:-}" == --locked && "${3:-}" == --path ]] || exit 2
source_path="$4"
[[ "${5:-}" == --root ]] || exit 2
root="$6"
[[ "$source_path" == */source/tools/harness-gate ]] || exit 2
mkdir -p "$root/bin"
if [[ "${HARNESS_GATE_TEST_PLATFORM:-linux}" == windows ]]; then
    printf 'source fixture windows binary\n' >"$root/bin/harness-gate.exe"
else
    printf 'source fixture binary\n' >"$root/bin/harness-gate"
fi
EOF
chmod 755 "$FAKE_BIN/cargo"

cat >"$FAKE_BIN/uname" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
if [[ "${HARNESS_GATE_TEST_PLATFORM:-linux}" == windows ]]; then
    case "${1:-}" in
        -s) printf 'MINGW64_NT\n' ;;
        -m) printf 'x86_64\n' ;;
        *) exit 2 ;;
    esac
else
    case "${1:-}" in
        -s) printf 'Linux\n' ;;
        -m) printf 'x86_64\n' ;;
        *) exit 2 ;;
    esac
fi
EOF
chmod 755 "$FAKE_BIN/uname"

cat >"$FAKE_BIN/cp" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
if [[ "${HARNESS_GATE_TEST_BLOCK_CP:-0}" == 1 && "${2:-}" == */.harness-gate.* && -n "${HARNESS_GATE_TEST_CP_STARTED:-}" ]]; then
    : >"$HARNESS_GATE_TEST_CP_STARTED"
    while [[ ! -e "${HARNESS_GATE_TEST_CP_RELEASE:-}" ]]; do
        sleep 0.05
    done
fi
exec /bin/cp "$@"
EOF
chmod 755 "$FAKE_BIN/cp"

run_installer() {
    local destination="$1"
    shift
    env \
        PATH="$FAKE_BIN:$PATH" \
        HARNESS_GATE_TEST_FIXTURE="$FIXTURE" \
        HARNESS_GATE_TEST_COSIGN_LOG="$TEMP_ROOT/cosign.log" \
        HARNESS_GATE_TEST_PLATFORM="${HARNESS_GATE_TEST_PLATFORM:-linux}" \
        HARNESS_GATE_TEST_BLOCK_CP="${HARNESS_GATE_TEST_BLOCK_CP:-0}" \
        HARNESS_GATE_TEST_CP_STARTED="${HARNESS_GATE_TEST_CP_STARTED:-}" \
        HARNESS_GATE_TEST_CP_RELEASE="${HARNESS_GATE_TEST_CP_RELEASE:-}" \
        bash "$ROOT/install.sh" --version v0.3.3 --install-dir "$destination" "$@"
}

mode_of() {
    if stat -c '%a' "$1" >/dev/null 2>&1; then
        stat -c '%a' "$1"
    else
        stat -f '%Lp' "$1"
    fi
}

assert_file_content() {
    local path="$1"
    [[ -f "$path" ]] || { printf 'missing expected file: %s\n' "$path" >&2; exit 1; }
    cmp -s "$FIXTURE/$binary_name" "$path" || {
        printf 'file content mismatch: %s\n' "$path" >&2
        exit 1
    }
}

install_dir="$TEMP_ROOT/install"
run_installer "$install_dir"
assert_file_content "$install_dir/harness-gate"
[[ "$(mode_of "$install_dir/harness-gate")" == 755 ]] || exit 1
[[ "$(wc -l <"$TEMP_ROOT/cosign.log")" -eq 2 ]] || exit 1
[[ -z "$(find "$install_dir" -maxdepth 1 -name '.harness-gate.*' -print -quit)" ]] || exit 1

first_inode="$(stat -c '%i' "$install_dir/harness-gate" 2>/dev/null || stat -f '%i' "$install_dir/harness-gate")"
printf 'old binary\n' >"$install_dir/harness-gate"
run_installer "$install_dir" >/dev/null
assert_file_content "$install_dir/harness-gate"
second_inode="$(stat -c '%i' "$install_dir/harness-gate" 2>/dev/null || stat -f '%i' "$install_dir/harness-gate")"
[[ "$first_inode" != "$second_inode" ]] || exit 1

tampered_dir="$TEMP_ROOT/tampered"
mkdir -m 700 "$tampered_dir"
printf 'old binary\n' >"$tampered_dir/harness-gate"
cp "$FIXTURE/$binary_name" "$FIXTURE/original-binary"
printf 'tampered fixture binary\n' >"$FIXTURE/$binary_name"
set +e
run_installer "$tampered_dir" >/dev/null 2>&1
tampered_status=$?
set -e
((tampered_status != 0)) || exit 1
grep -Fxq 'old binary' "$tampered_dir/harness-gate" || exit 1
mv "$FIXTURE/original-binary" "$FIXTURE/$binary_name"

signature_dir="$TEMP_ROOT/signature-failure"
mkdir -m 700 "$signature_dir"
printf 'old binary\n' >"$signature_dir/harness-gate"
set +e
HARNESS_GATE_TEST_COSIGN_FAIL=1 run_installer "$signature_dir" >/dev/null 2>&1
signature_status=$?
set -e
((signature_status != 0)) || exit 1
grep -Fxq 'old binary' "$signature_dir/harness-gate" || exit 1

symlink_dir="$TEMP_ROOT/symlink"
mkdir -m 700 "$symlink_dir"
outside="$TEMP_ROOT/outside"
printf 'outside\n' >"$outside"
ln -s "$outside" "$symlink_dir/harness-gate"
set +e
run_installer "$symlink_dir" >/dev/null 2>&1
symlink_status=$?
set -e
((symlink_status != 0)) || exit 1
grep -Fxq 'outside' "$outside" || exit 1

parent_symlink="$TEMP_ROOT/parent-link"
ln -s "$TEMP_ROOT" "$parent_symlink"
set +e
run_installer "$parent_symlink/parent-install" >/dev/null 2>&1
parent_status=$?
set -e
((parent_status != 0)) || exit 1

source_dir="$TEMP_ROOT/source-install"
run_installer "$source_dir" --from-source >/dev/null
[[ -f "$source_dir/harness-gate" ]] || exit 1
grep -Fxq 'source fixture binary' "$source_dir/harness-gate" || exit 1
[[ "$(mode_of "$source_dir/harness-gate")" == 755 ]] || exit 1

wrong_identity_dir="$TEMP_ROOT/wrong-identity"
mkdir -m 700 "$wrong_identity_dir"
printf 'old binary\n' >"$wrong_identity_dir/harness-gate"
printf 'identity=https://example.invalid/wrong\n' >"$FIXTURE/$binary_name.crt"
set +e
run_installer "$wrong_identity_dir" >/dev/null 2>&1
wrong_identity_status=$?
set -e
((wrong_identity_status != 0)) || exit 1
grep -Fxq 'old binary' "$wrong_identity_dir/harness-gate" || exit 1
printf 'identity=https://github.com/musutrade/Harness-Gate/.github/workflows/release.yml@refs/tags/v0.3.3\n' >"$FIXTURE/$binary_name.crt"

windows_dir="$TEMP_ROOT/windows-install"
HARNESS_GATE_TEST_PLATFORM=windows run_installer "$windows_dir" >/dev/null
[[ -f "$windows_dir/harness-gate.exe" ]] || exit 1
[[ ! -e "$windows_dir/harness-gate" ]] || exit 1
cmp -s "$FIXTURE/$windows_binary_name" "$windows_dir/harness-gate.exe" || exit 1

signal_dir="$TEMP_ROOT/signal-install"
mkdir -m 700 "$signal_dir"
printf 'old binary\n' >"$signal_dir/harness-gate"
signal_started="$TEMP_ROOT/signal-cp-started"
signal_release="$TEMP_ROOT/signal-cp-release"
set +e
HARNESS_GATE_TEST_BLOCK_CP=1 \
HARNESS_GATE_TEST_CP_STARTED="$signal_started" \
HARNESS_GATE_TEST_CP_RELEASE="$signal_release" \
env \
    PATH="$FAKE_BIN:$PATH" \
    HARNESS_GATE_TEST_FIXTURE="$FIXTURE" \
    HARNESS_GATE_TEST_COSIGN_LOG="$TEMP_ROOT/cosign.log" \
    HARNESS_GATE_TEST_PLATFORM=linux \
    HARNESS_GATE_TEST_BLOCK_CP=1 \
    HARNESS_GATE_TEST_CP_STARTED="$signal_started" \
    HARNESS_GATE_TEST_CP_RELEASE="$signal_release" \
    bash "$ROOT/install.sh" --version v0.3.3 --install-dir "$signal_dir" >/dev/null 2>&1 &
signal_pid=$!
signal_ready=0
for _ in {1..100}; do
    if [[ -e "$signal_started" ]]; then
        signal_ready=1
        break
    fi
    if ! kill -0 "$signal_pid" 2>/dev/null; then
        break
    fi
    sleep 0.05
done
if ((signal_ready == 1)); then
    kill -TERM "$signal_pid" 2>/dev/null || true
fi
: >"$signal_release"
wait "$signal_pid"
signal_status=$?
set -e
((signal_ready == 1 && signal_status == 143)) || exit 1
grep -Fxq 'old binary' "$signal_dir/harness-gate" || exit 1
[[ -z "$(find "$signal_dir" -maxdepth 1 -name '.harness-gate.*' -print -quit)" ]] || exit 1

missing_dir="$TEMP_ROOT/missing-asset"
mkdir -m 700 "$missing_dir"
printf 'old binary\n' >"$missing_dir/harness-gate"
mv "$FIXTURE/$binary_name.sig" "$FIXTURE/$binary_name.sig.missing"
set +e
run_installer "$missing_dir" >/dev/null 2>&1
missing_status=$?
set -e
((missing_status != 0)) || exit 1
grep -Fxq 'old binary' "$missing_dir/harness-gate" || exit 1
mv "$FIXTURE/$binary_name.sig.missing" "$FIXTURE/$binary_name.sig"

unsafe_dir="$TEMP_ROOT/unsafe-permissions"
mkdir -m 0777 "$unsafe_dir"
chmod 0777 "$unsafe_dir"
printf 'old binary\n' >"$unsafe_dir/harness-gate"
set +e
run_installer "$unsafe_dir" >/dev/null 2>&1
unsafe_status=$?
set -e
((unsafe_status != 0)) || exit 1
grep -Fxq 'old binary' "$unsafe_dir/harness-gate" || exit 1

for invalid_version in v1.2 v01.2.3 v1.2.3-01; do
    set +e
    env PATH="$FAKE_BIN:$PATH" bash "$ROOT/install.sh" \
        --version "$invalid_version" --install-dir "$TEMP_ROOT/version" \
        >/dev/null 2>&1
    invalid_status=$?
    set -e
    ((invalid_status != 0)) || exit 1
done

printf 'installer integrity tests: pass\n'
