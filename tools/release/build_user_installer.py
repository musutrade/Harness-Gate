"""Prepare user delivery alongside the existing protected collector publication.

The resulting installer is a separately provisioned trust root, like install.sh.
It embeds exact bootstrap, verifier and release pins, never learns them from an
untrusted plugin. Existing signed release bytes are retained unchanged.
"""
import argparse
import base64
import gzip
import json
from pathlib import Path
import shutil

import build_collector_bootstrap as bootstrap
import collector_assets as assets
import collector_transport as transport


TEMPLATE = '''#!/usr/bin/env bash
set -Eeuo pipefail
umask 077
if [[ "${1:-}" == --help || "${1:-}" == -h ]]; then
    echo 'Install the optional Harness-Gate Rust plugin.'
    echo 'Usage: install-rust.sh [--root DIR] [--cache-dir DIR] [--offline DIR]'
    echo 'Downloads are verified and cached; rerunning verifies the existing installation.'
    exit 0
fi
[[ "$(uname -s)/$(uname -m)" == Linux/x86_64 ]] || { echo 'This Rust plugin supports Linux x86_64 only.' >&2; exit 1; }
offline=''
args=("$@")
for ((i=0;i<${#args[@]};i++)); do
    if [[ "${args[i]}" == --offline ]]; then
        offline="${args[i+1]:?--offline requires a directory}"
    fi
done
unset PYTHONHOME PYTHONPATH PYTHONSTARTUP LD_PRELOAD LD_LIBRARY_PATH
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
if [[ -n "$offline" ]]; then
    cp -- "$offline/@BOOTSTRAP@" "$stage/bootstrap.tar.gz"
else
    curl --fail --show-error --location --proto '=https' --proto-redir '=https' --http1.1 \\
        --retry 5 --retry-all-errors --connect-timeout 15 \\
        '@BASE@/@BOOTSTRAP@' -o "$stage/bootstrap.tar.gz"
fi
echo '@SHA@  '"$stage/bootstrap.tar.gz" | sha256sum --check --status || { echo 'Installer checksum mismatch' >&2; exit 1; }
mkdir "$stage/runtime"
tar -xzf "$stage/bootstrap.tar.gz" -C "$stage/runtime" --no-same-owner --no-same-permissions
cat > "$stage/catalog.json" <<'HARNESS_GATE_CATALOG'
@CATALOG@
HARNESS_GATE_CATALOG
export LD_LIBRARY_PATH="$stage/runtime/lib"
"$stage/runtime/python/bin/python3" -I -S -B "$stage/runtime/tools/release/friendly_collector_install.py" \\
    --catalog "$stage/catalog.json" "$@"
'''


def build(release, runtime, trust, output, base_url, prepared_transport=None):
    assets.require(base_url.startswith('https://') and "'" not in base_url and '\n' not in base_url, 'invalid installer release URL')
    manifest = assets.read(release / 'manifest.json')
    version = manifest['collector']['version']
    assets.verify(release, trust, 'rust-collector-v' + version)
    if prepared_transport:
        output.mkdir(parents=True, exist_ok=False)
        for path in prepared_transport.iterdir():
            assets.regular(path); shutil.copyfile(path, output / path.name)
        descriptor = assets.read(output / 'transport.json')
        assets.require(descriptor['archive_sha256'] == assets.sha(release / 'collector.tar'), 'wrong prepared transport')
    else:
        descriptor = transport.pack(release / 'collector.tar', output)
    capsule = output / 'installer-bootstrap.tar'
    receipt = bootstrap.build(runtime, capsule, user_entry=True)
    compressed = output / 'installer-bootstrap.tar.gz'
    with capsule.open('rb') as src, compressed.open('xb') as dst:
        with gzip.GzipFile(filename='', fileobj=dst, mode='wb', mtime=0) as gz:
            shutil.copyfileobj(src, gz)
    capsule.unlink()
    def row(path):
        return {'name': path.name, 'sha256': assets.sha(path), 'size': path.stat().st_size}
    controls = []
    for name in assets.ASSETS + assets.CONTROL:
        if name == 'collector.tar': continue
        shutil.copyfile(release / name, output / name)
        controls.append(row(output / name))
    host = dict(trust)
    # Host paths are reviewed inputs; release payloads cannot choose them.
    profile = {'host': host, **{key: base64.b64encode(Path(trust[key]).read_bytes()).decode()
                              for key in ('public_key', 'trusted_root')}}
    profile['verifier'] = row(Path(trust['cosign'])) | {
        'name': 'cosign-linux-amd64',
        'url': 'https://github.com/sigstore/cosign/releases/download/v3.1.3/cosign-linux-amd64'}
    catalog = {'schema': 'rust-collector-user-install/v1', 'version': version,
               'base_url': base_url, 'release_url': base_url,
               'archive_sha256': descriptor['archive_sha256'], 'host_abi': manifest['host_abi'],
               'transport': row(output / 'transport.json'), 'trust': profile, 'controls': controls}
    script = TEMPLATE.replace('@CATALOG@', json.dumps(catalog, sort_keys=True))
    script = script.replace('@BASE@', base_url).replace('@BOOTSTRAP@', compressed.name).replace('@SHA@', assets.sha(compressed))
    (output / 'install-rust.sh').write_text(script)
    (output / 'install-rust.sh').chmod(0o755)
    assets.write(output / 'installer-build.json', {'schema': 'rust-collector-user-installer-build/v1',
        'original_archive_sha256': descriptor['archive_sha256'], 'bootstrap': receipt,
        'download_bytes': sum(b['size'] for b in descriptor['bundles']) + compressed.stat().st_size,
        'installer': row(output / 'install-rust.sh'), 'catalog': catalog})
    return catalog


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('release', 'runtime', 'trust', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--base-url', required=True)
    parser.add_argument('--prepared-transport', type=Path)
    args = parser.parse_args()
    build(args.release, args.runtime, assets.read(args.trust), args.output, args.base_url, args.prepared_transport)
