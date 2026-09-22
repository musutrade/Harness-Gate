#!/usr/bin/env python3
"""Build, test and clean-install independently versioned collector packages."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[2]


def run(*args: str, cwd: Path = ROOT, env=None) -> str:
    result = subprocess.run(args, cwd=cwd, env=env, check=True, text=True,
                            stdout=subprocess.PIPE)
    print(result.stdout, end="", flush=True)
    return result.stdout


def node_package(directory: str, output: Path) -> None:
    source = ROOT / "tools/quality" / directory
    package = json.loads((source / "package.json").read_text())
    run("npm", "ci", "--ignore-scripts", cwd=source)
    run("npm", "test", cwd=source)
    if directory == "typescript-risk":
        run("npm", "run", "test:core", cwd=source)
    packed = json.loads(run("npm", "pack", "--ignore-scripts", "--json",
                            "--pack-destination", str(output), cwd=source))
    archive = output / packed[0]["filename"]
    # A fresh install catches omitted runtime modules and shrinkwrap errors.
    with tempfile.TemporaryDirectory(prefix="collector-consumer-") as tmp:
        consumer = Path(tmp)
        (consumer / "package.json").write_text(json.dumps({
            "name": "harness-gate-collector-runtime", "version": package["version"],
            "private": True,
        }))
        run("npm", "install", "--ignore-scripts", "--omit=dev", "--prefix", tmp,
            str(archive))
        executable = consumer / "node_modules/.bin" / next(iter(package["bin"]))
        version = run(str(executable), "--version")
        if package["version"] not in version:
            raise RuntimeError("installed collector version mismatch")
        # Generate the actual installed runtime dependency inventory.
        sbom = run("npm", "sbom", "--sbom-format=cyclonedx", "--omit=dev",
                   cwd=consumer)
        json.loads(sbom)
        (output / (archive.name + ".sbom.cdx.json")).write_text(sbom)


def rust_package(output: Path) -> None:
    source = ROOT / "tools/quality/rust-source-risk"
    manifest = source / "ast/Cargo.toml"
    identity = json.loads((source / "plugin.json").read_text())
    with tempfile.TemporaryDirectory(prefix="rust-source-package-") as tmp:
        stage = Path(tmp)
        env = dict(os.environ, CARGO_TARGET_DIR=str(stage / "target"))
        run("cargo", "test", "--locked", "--manifest-path", str(manifest), env=env)
        run("cargo", "build", "--locked", "--release", "--manifest-path", str(manifest), env=env)
        inventory = stage / "target/release/harness-gate-rust-source-inventory"
        env["RUST_SOURCE_INVENTORY"] = str(inventory)
        # Existing tests also exercise the default sibling executable path.
        shutil.copy2(inventory, source / "inventory")
        run("python3", "-m", "unittest", "discover", "-s", str(source),
            "-p", "test_*.py", "-v", env=env)
        name = "harness-gate-rust-source-risk-" + identity["version"] + "-linux-amd64"
        package = stage / name
        package.mkdir()
        for filename in ("plugin.py", "measure.py", "capture.py", "plugin.json", "LICENSE", "README.md"):
            shutil.copy2(source / filename, package / filename)
        shutil.copy2(inventory, package / "inventory")
        run("python3", str(package / "plugin.py"), "--help", cwd=stage)
        with tarfile.open(output / (name + ".tar.gz"), "w:gz") as archive:
            archive.add(package, arcname=name)
        metadata = stage / "cargo-metadata.json"
        metadata.write_text(run("cargo", "metadata", "--locked", "--format-version", "1",
                                "--manifest-path", str(manifest), env=env))
        run("python3", "tools/release/generate-sbom.py", "--metadata", str(metadata),
            "--lockfile", str(source / "ast/Cargo.lock"),
            "--output", str(output / (name + ".tar.gz.sbom.cdx.json")))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    node_package("typescript-risk", output)
    node_package("http-json-contract", output)
    rust_package(output)


if __name__ == "__main__":
    main()
