"""Dependency capabilities for new signed runtimes; build-host facts are diagnostic.

Only assembly uses readelf. Installation/runtime probes use the private Python
runtime, available host libraries and shell; no distro or kernel equality test.
"""
from __future__ import annotations

import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess

SCHEMA = 'rust-runtime-requirements/v1'
HOST_LIBRARIES = {'libc.so.6', 'libm.so.6', 'libpthread.so.0', 'libdl.so.2',
                  'librt.so.1', 'ld-linux-x86-64.so.2'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def version(value):
    require(isinstance(value, str) and re.fullmatch(r'[0-9]+\.[0-9]+(?:\.[0-9]+)?', value),
            'invalid glibc dependency version')
    parts = tuple(map(int, value.split('.')))
    return parts + (0,) * (3 - len(parts))


def validate(requirements):
    require(isinstance(requirements, dict) and set(requirements) ==
            {'schema', 'system', 'machine', 'libc_min', 'libraries', 'commands'},
            'missing/unknown runtime dependency requirements')
    require(requirements['schema'] == SCHEMA and requirements['system'] == 'Linux'
            and requirements['machine'] == 'x86_64', 'unsupported dependency platform')
    require(version(requirements['libc_min']) >= (2, 17, 0), 'unsupported glibc dependency baseline')
    libraries = requirements['libraries']
    require(isinstance(libraries, list) and all(isinstance(n, str) for n in libraries)
            and len(libraries) == len(set(libraries)) and 'libc.so.6' in libraries
            and set(libraries) <= HOST_LIBRARIES, 'invalid host library requirements')
    require(requirements['commands'] == ['sh'], 'invalid runtime command requirements')


def check_abi(requirements, observed):
    """Check capability bounds, never kernel or build-host library fingerprints."""
    validate(requirements)
    require(observed.get('target') == 'x86_64-unknown-linux-gnu',
            'dependency unavailable: requires Linux x86_64 with glibc')
    raw = observed.get('glibc', '')
    require(isinstance(raw, str) and raw.startswith('glibc '), 'dependency unavailable: glibc is required')
    require(version(raw[6:]) >= version(requirements['libc_min']),
            f"dependency incompatible: glibc >= {requirements['libc_min']} required, found {raw}; "
            'select a component build with a compatible libc baseline')


def probe(requirements):
    validate(requirements)
    target = 'x86_64-unknown-linux-gnu' if (platform.system(), platform.machine()) == ('Linux', 'x86_64') else ''
    observed = {'target': target, 'glibc': os.confstr('CS_GNU_LIBC_VERSION') or '',
                'kernel': platform.release()}
    check_abi(requirements, observed)
    require(shutil.which('sh', path=os.defpath) is not None,
            'dependency missing: POSIX sh; install a compatible shell')
    libraries = {}
    handles = []
    for name in requirements['libraries']:
        try:
            handles.append(ctypes.CDLL(name))
        except OSError as error:
            raise ValueError(f'dependency missing/incompatible: {name}: {error}; install a compatible libc runtime') from error
    # Retain actual library identity for capture/replay. Only installation stops
    # comparing it with the publisher's fingerprint; historical capture identity
    # must still distinguish a patched library with the same glibc version.
    for line in Path('/proc/self/maps').read_text().splitlines():
        fields = line.split(maxsplit=5)
        if len(fields) != 6 or not fields[5].startswith('/'):
            continue
        path = Path(fields[5])
        if path.name in requirements['libraries'] and path.name not in libraries:
            with path.open('rb') as stream:
                libraries[path.name] = hashlib.file_digest(stream, 'sha256').hexdigest()
    require(set(libraries) == set(requirements['libraries']),
            'cannot identify loaded libc dependencies through /proc/self/maps')
    # Runtime facts remain observable, but package compatibility does not depend
    # on their exact bytes. Actual private tool hashes remain pinned separately.
    observed['runtime_dependencies_sha256'] = hashlib.sha256(json.dumps(
        {'glibc': observed['glibc'], 'libraries': libraries}, sort_keys=True).encode()).hexdigest()
    return observed


def requirements_for_manifest(manifest):
    if manifest.get('schema') == 'rust-collector-delivery/v3':
        result = manifest.get('runtime_requirements')
        validate(result)
        return result
    require('runtime_requirements' not in manifest, 'legacy manifest cannot override host compatibility')
    return None


def elf_requirements(output):
    """Read NEEDED names and *required* versions, never exported definitions."""
    libraries = re.findall(r'\(NEEDED\).*?\[([^\]]+)\]', output)
    needs = output.split('Version needs section', 1)
    versions = re.findall(r'Name: GLIBC_([0-9]+\.[0-9]+(?:\.[0-9]+)?)\b', needs[1]) if len(needs) == 2 else []
    return libraries, versions


def derive(root):
    """Publisher-only capability inventory from actual assembled ELF bytes."""
    libraries, versions, bundled = set(), ['2.17'], set()
    elf_files = []
    for path in sorted(root.rglob('*')):
        if not path.is_file():
            continue
        # The link sysroot contains SDK inputs, not loaded runtime libraries.
        # Generated C/Rust executables are checked by the install self-test.
        if path.is_relative_to(root / 'link/sysroot'):
            continue
        bundled.add(path.name)
        with path.open('rb') as stream:
            if stream.read(4) == b'\x7fELF':
                elf_files.append(path)
    require(bool(elf_files), 'cannot derive dependencies without ELF payloads')
    for path in elf_files:
        result = subprocess.run(['readelf', '--wide', '--dynamic', '--version-info', str(path)],
                                capture_output=True, text=True, env={**os.environ, 'LC_ALL': 'C'}, timeout=30)
        require(result.returncode == 0, 'dependency inventory failed: ' + str(path))
        needed, required_versions = elf_requirements(result.stdout)
        require(all(name in HOST_LIBRARIES or name in bundled for name in needed),
                'unpackaged runtime dependencies: ' + ', '.join(n for n in needed if n not in HOST_LIBRARIES and n not in bundled))
        libraries.update(name for name in needed if name in HOST_LIBRARIES)
        versions.extend(required_versions)
    libraries.add('libc.so.6')
    result = {'schema': SCHEMA, 'system': 'Linux', 'machine': 'x86_64',
              'libc_min': max(versions, key=version), 'libraries': sorted(libraries), 'commands': ['sh']}
    validate(result)
    return result
