#!/usr/bin/env python3
"""Host-trusted, offline Linux collector lifecycle. No payload code is executed."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import fcntl
import json
import os
from pathlib import Path
import shutil
import stat
import tarfile
import uuid

import collector_assets as assets

require = assets.require
DELIVERY = '.delivery'


def sync(directory):
    descriptor = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def atomic(path, value):
    temporary = path.parent / ('.write-' + path.name)
    if temporary.exists() or temporary.is_symlink():
        assets.regular(temporary)
        temporary.unlink()
    with temporary.open('x') as stream:
        json.dump(value, stream, sort_keys=True)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)
    sync(path.parent)


def directories(names):
    return {str(p) for name in names for p in Path(name).parents if str(p) != '.'}


def tree_check(root, names, *, complete):
    """Never traverse links. Undeclared content blocks removal as well as install."""
    expected, dirs = set(names), directories(names)
    found = set()
    for current, children, files in os.walk(root, followlinks=False):
        for name in children + files:
            path = Path(current) / name
            relative = str(path.relative_to(root))
            mode = path.lstat()
            require(not stat.S_ISLNK(mode.st_mode), 'unsafe installed link')
            if stat.S_ISDIR(mode.st_mode):
                require(relative in dirs, 'extra installed directory')
            else:
                assets.regular(path)
                require(relative in expected, 'extra installed payload')
                found.add(relative)
    require(not complete or found == expected, 'missing installed payload')


def remove_owned(root, names):
    if not root.exists():
        return
    require(not root.is_symlink() and root.is_dir(), 'unsafe transaction directory')
    tree_check(root, names, complete=False)
    for name in names:
        (root / name).unlink(missing_ok=True)
    for name in sorted(directories(names), key=lambda n: len(Path(n).parts), reverse=True):
        path = root / name
        if path.exists():
            path.rmdir()
    root.rmdir()
    sync(root.parent)


def recover(root):
    journal = root / 'transaction.json'
    if not journal.exists():
        return
    assets.regular(journal)
    value = assets.read(journal)
    relative = assets.safe_name(value['directory'])
    parts = Path(relative).parts
    require(len(parts) == 2 and parts[0] in ('staging', 'versions'), 'invalid recovery location')
    names = [assets.safe_name(n) for n in value['files']]
    remove_owned(root / relative, names)
    journal.unlink()
    sync(root)


@contextmanager
def locked(root):
    require(root.is_absolute(), 'installation root must be absolute')
    for path in [*reversed(root.parents), root]:
        require(not path.is_symlink(), 'unsafe installation root link')
    root.mkdir(mode=0o700, parents=True, exist_ok=True)
    mode = root.stat()
    require(mode.st_uid == os.getuid() and mode.st_mode & 0o077 == 0, 'installation root must be private (0700)')
    descriptor = os.open(root / 'lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    try:
        mode = os.fstat(descriptor)
        require(stat.S_ISREG(mode.st_mode) and mode.st_nlink == 1, 'unsafe lock')
        fcntl.flock(descriptor, fcntl.LOCK_EX)
        for name in ('staging', 'versions'):
            path = root / name
            require(not path.is_symlink(), 'unsafe lifecycle directory')
            path.mkdir(mode=0o700, exist_ok=True)
        recover(root)
        yield
    finally:
        os.close(descriptor)


def payload_names(manifest):
    names = [assets.safe_name(p['path']) for p in manifest['payloads']]
    require(all(n != DELIVERY and not n.startswith(DELIVERY + '/') for n in names), 'reserved payload path')
    require(not set(names) & directories(names), 'payload file/directory collision')
    require('bin/harness-gate-rust-collector' in names, 'missing collector launcher')
    return names


def extract(archive_path, destination, manifest):
    names = payload_names(manifest)
    expected = {p['path']: p['sha256'] for p in manifest['payloads']}
    dirs, seen = directories(names), set()
    with tarfile.open(archive_path, 'r:') as archive:
        for member in archive:
            name = assets.safe_name(member.name)
            require(name not in seen, 'duplicate archive member')
            seen.add(name)
            require(not member.pax_headers and not member.issparse(), 'extended archive metadata rejected')
            if member.isdir():
                require(name in dirs and member.mode & ~0o777 == 0, 'extra/unsafe archive directory')
                continue
            require(member.isfile() and name in expected and member.mode in (0o644, 0o755),
                    'unsafe link/type/mode or extra archive payload')
            require(name != 'bin/harness-gate-rust-collector' or member.mode == 0o755,
                    'collector launcher must be executable')
            path = destination / name
            path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
            with archive.extractfile(member) as source, path.open('xb') as output:
                shutil.copyfileobj(source, output)
                os.fchmod(output.fileno(), member.mode)
                output.flush()
                os.fsync(output.fileno())
            require(assets.sha(path) == expected[name], 'payload checksum mismatch')
        archive.fileobj.seek(archive.offset)
        while block := archive.fileobj.read(1024 * 1024):
            require(not any(block), 'extra data after archive terminator')
    require(set(names) <= seen, 'missing archive payload')
    for name in sorted(dirs, key=lambda n: len(Path(n).parts), reverse=True):
        sync(destination / name)
    sync(destination)


def installed(root, selected, trust):
    path = root / 'versions' / assets.version(selected)
    require(path.is_dir() and not path.is_symlink(), 'version not installed')
    require(not (path / DELIVERY).is_symlink(), 'unsafe delivery metadata link')
    manifest = assets.verify(path / DELIVERY, trust, 'rust-collector-v' + selected)
    names = payload_names(manifest) + [DELIVERY + '/' + n for n in assets.ASSETS + assets.CONTROL]
    tree_check(path, names, complete=True)
    for row in manifest['payloads']:
        require(assets.sha(path / row['path']) == row['sha256'], 'installed payload checksum mismatch')
    with tarfile.open(path / DELIVERY / 'collector.tar', 'r:') as archive:
        for member in archive:
            if member.isfile():
                require(stat.S_IMODE((path / assets.safe_name(member.name)).stat().st_mode) == member.mode,
                        'installed payload mode mismatch')
    return path, names


def install(root, release, trust, tag, checkpoint=lambda _: None):
    with locked(root):
        # Snapshot only fixed release names. Verification operates on private bytes.
        require(set(p.name for p in release.iterdir()) == set(assets.ASSETS + assets.CONTROL),
                'missing/extra release assets')
        stage_name = 'staging/' + uuid.uuid4().hex
        stage = root / stage_name
        controls = [DELIVERY + '/' + n for n in assets.ASSETS + assets.CONTROL]
        atomic(root / 'transaction.json', {'directory': stage_name, 'files': controls})
        (stage / DELIVERY).mkdir(parents=True, mode=0o700)
        for name in assets.ASSETS + assets.CONTROL:
            assets.regular(release / name)
            shutil.copyfile(release / name, stage / DELIVERY / name)
            with (stage / DELIVERY / name).open('rb') as stream:
                os.fsync(stream.fileno())
        sync(stage / DELIVERY)
        manifest = assets.verify(stage / DELIVERY, trust, tag)
        selected = assets.version(manifest['collector']['version'])
        destination = root / 'versions' / selected
        require(not destination.exists() and not destination.is_symlink(), 'version already installed; use select')
        names = payload_names(manifest) + controls
        atomic(root / 'transaction.json', {'directory': stage_name, 'files': names})
        checkpoint('verified')
        extract(stage / DELIVERY / 'collector.tar', stage, manifest)
        tree_check(stage, names, complete=True)
        checkpoint('extracted')
        os.rename(stage, destination)
        sync(root / 'versions')
        sync(root / 'staging')
        checkpoint('committed')
        atomic(root / 'current.json', {'version': selected})
        checkpoint('activated')
        (root / 'transaction.json').unlink()
        sync(root)
        return destination / 'bin/harness-gate-rust-collector'


def select(root, selected, trust):
    """Explicit selection/rollback rechecks signature, ABI and every installed byte."""
    with locked(root):
        path, _ = installed(root, selected, trust)
        atomic(root / 'current.json', {'version': selected})
        return path / 'bin/harness-gate-rust-collector'


def uninstall(root, selected, trust):
    with locked(root):
        path, names = installed(root, selected, trust)
        current = root / 'current.json'
        if current.exists():
            assets.regular(current)
            if assets.read(current)['version'] == selected:
                current.unlink()
                sync(root)
        atomic(root / 'transaction.json', {'directory': str(path.relative_to(root)), 'files': names})
        recover(root)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--trust', type=Path, required=True)
    sub = parser.add_subparsers(dest='action', required=True)
    command = sub.add_parser('install')
    command.add_argument('--release', type=Path, required=True)
    command.add_argument('--tag', required=True)
    for action in ('select', 'rollback', 'uninstall'):
        sub.add_parser(action).add_argument('--version', required=True)
    sub.add_parser('recover')
    args = parser.parse_args()
    trust = assets.read(args.trust)
    if args.action == 'install':
        print(install(args.root, args.release, trust, args.tag))
    elif args.action in ('select', 'rollback'):
        print(select(args.root, args.version, trust))
    elif args.action == 'uninstall':
        uninstall(args.root, args.version, trust)
    else:
        with locked(args.root):
            pass


if __name__ == '__main__':
    main()
