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
sha256sum "$FIXTURE/$binary_name" | sed "s#  $FIXTURE/#  #" >"$FIXTURE/SHA256SUMS"
printf 'sigstore manifest fixture\n' >"$FIXTURE/SHA256SUMS.sig"
printf 'sigstore manifest certificate fixture\n' >"$FIXTURE/SHA256SUMS.crt"
printf 'sigstore binary fixture\n' >"$FIXTURE/$binary_name.sig"
printf 'sigstore binary certificate fixture\n' >"$FIXTURE/$binary_name.crt"

cat >"$FAKE_BIN/curl" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
output=""
url=""
expect_output=0
for argument in "$@"; do
    if ((expect_output)); then
        output="$argument"
        expect_output=0
    elif [[ "$argument" == --output ]]; then
        expect_output=1
    elif [[ "$argument" == --output=* ]]; then
        output="${argument#--output=}"
    elif [[ "$argument" == https://* ]]; then
        url="$argument"
    fi
done
[[ -n "$output" && -n "$url" ]] || exit 2
source_file="$HARNESS_GATE_TEST_FIXTURE/$(basename "$url")"
[[ -f "$source_file" ]] || exit 22
cp "$source_file" "$output"
EOF
chmod 755 "$FAKE_BIN/curl"

cat >"$FAKE_BIN/cosign" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
printf '%s\n' "$*" >>"$HARNESS_GATE_TEST_COSIGN_LOG"
if [[ "${HARNESS_GATE_TEST_COSIGN_FAIL:-0}" == 1 ]]; then
    exit 1
fi
exit 0
EOF
chmod 755 "$FAKE_BIN/cosign"

cat >"$FAKE_BIN/git" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
if [[ "$1" == clone ]]; then
    destination="${@: -1}"
    mkdir -p "$destination/tools/harness-gate"
    exit 0
fi
if [[ "$1" == -C && "$3" == diff ]]; then
    exit 0
fi
exit 2
EOF
chmod 755 "$FAKE_BIN/git"

cat >"$FAKE_BIN/cargo" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
root=""
expect_root=0
for argument in "$@"; do
    if ((expect_root)); then
        root="$argument"
        expect_root=0
    elif [[ "$argument" == --root ]]; then
        expect_root=1
    fi
done
[[ -n "$root" ]] || exit 2
mkdir -p "$root/bin"
printf 'source fixture binary\n' >"$root/bin/harness-gate"
EOF
chmod 755 "$FAKE_BIN/cargo"

run_installer() {
    local destination="$1"
    shift
    env \
        PATH="$FAKE_BIN:$PATH" \
        HARNESS_GATE_TEST_FIXTURE="$FIXTURE" \
        HARNESS_GATE_TEST_COSIGN_LOG="$TEMP_ROOT/cosign.log" \
        bash "$ROOT/install.sh" --version v0.3.3 --install-dir "$destination" "$@"
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
[[ "$(stat -c '%a' "$install_dir/harness-gate")" == 755 ]] || exit 1
[[ "$(wc -l <"$TEMP_ROOT/cosign.log")" -eq 2 ]] || exit 1
grep -Fq -- '--certificate-oidc-issuer https://token.actions.githubusercontent.com' "$TEMP_ROOT/cosign.log" || exit 1
grep -Fq -- 'refs/tags/v0\.3\.3' "$TEMP_ROOT/cosign.log" || exit 1
[[ -z "$(find "$install_dir" -maxdepth 1 -name '.harness-gate.*' -print -quit)" ]] || exit 1

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
[[ "$(stat -c '%a' "$source_dir/harness-gate")" == 755 ]] || exit 1

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
