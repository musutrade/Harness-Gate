"""Publish a new installer version from exact reviewed inputs in the existing workflow."""
import argparse
import json
import os
from pathlib import Path
import subprocess

import build_user_installer as builder
import collector_assets as assets
import collector_release_policy as policy
import production_collector_release as production


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--packet', type=Path, required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--source', required=True)
    args = parser.parse_args()
    packet = assets.read(production.pinned(args.packet, args.sha256))
    assets.require(packet['schema'] == 'rust-collector-installer-approval/v1', 'wrong installer approval')
    assets.require(packet['source'] == args.source, 'wrong installer source')
    client = policy.core.GitHubClient('https://api.github.com', assets.REPOSITORY, os.environ['GH_TOKEN'])
    for ref in ('HEAD', 'refs/remotes/origin/main'):
        assets.require(policy.core._resolve_commit(Path.cwd(), ref, ref) == args.source, 'installer must use protected main')
    policy.core.verify_ci_run(client, args.source, 'main')
    policy.protected_environment(client.get_json('/repos/' + assets.REPOSITORY + '/environments/' + policy.ENVIRONMENT))
    trust = assets.read(production.pinned(Path(packet['trust']['path']), packet['trust']['sha256']))
    for name in ('openssl','cosign','public_key','trusted_root'):
        production.pinned(Path(trust[name]),trust[name+'_sha256'])
    release = Path(packet['release'])
    for name, digest in packet['signed_assets'].items(): production.pinned(release/name,digest)
    tag = 'rust-collector-installer-v' + assets.version(packet['version'])
    output = Path('target/collector-user-installer')
    catalog = builder.build(release, Path(packet['runtime']), trust, output,
                            'https://github.com/' + assets.REPOSITORY + '/releases/download/' + tag)
    production.pinned(output/'install-rust.sh',packet['installer_sha256'])
    production.create_or_verify_tag(tag,args.source)
    subprocess.run([trust['cosign'],'sign-blob','--yes','--bundle',str(output/'install-rust.sh.sigstore.json'),
                    str(output/'install-rust.sh')],check=True)
    # The publication source and operator-reviewed script pin authenticate the
    # bootstrap and transport. Underlying collector signatures remain unchanged.
    expected = {p.name:assets.sha(p) for p in output.iterdir()}
    notes = ('One-command Rust plugin installer. Download install-rust.sh and run bash install-rust.sh. '
             'The first install downloads compressed tools; later plugin versions reuse unchanged tools. '
             'For an offline kit also provision the pinned cosign verifier listed in installer-build.json.\n\n'
             'Installer version '+packet['version']+' delivers the unchanged signed Rust plugin '+catalog['version']+'. '
             'Supported host: '+json.dumps(catalog['host_abi'],sort_keys=True)+'.\n\n'
             'Corresponding source, full notices and relink materials: '
             'https://github.com/'+assets.REPOSITORY+'/releases/tag/rust-collector-materials-7165558-v1 .\n\n'
             'This product includes software developed by the NetBSD Foundation, Inc. and its contributors.')
    body = output.parent/'installer-release-notes.md';body.write_text(notes)
    subprocess.run(['gh','release','create',tag,'--repo',assets.REPOSITORY,'--verify-tag','--draft','--prerelease',
                    '--title',tag,'--notes-file',str(body),*[str(output/name) for name in sorted(expected)]],check=True)
    rows=json.loads(subprocess.check_output(['gh','api','repos/'+assets.REPOSITORY+'/releases?per_page=30']))
    draft=next(row for row in rows if row['tag_name']==tag)
    assets.require(draft['draft'] and {a['name']:a['digest'] for a in draft['assets']}==
                   {n:'sha256:'+d for n,d in expected.items()},'uploaded installer assets differ')
    subprocess.run(['gh','release','edit',tag,'--repo',assets.REPOSITORY,'--draft=false','--latest=false'],check=True)
    assets.write(output.parent/'installer-publication.json',{'status':'pass','source':args.source,'tag':tag,
                 'installer_sha256':packet['installer_sha256'],'assets':expected})


if __name__=='__main__': main()
