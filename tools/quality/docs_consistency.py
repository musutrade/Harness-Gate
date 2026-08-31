#!/usr/bin/env python3
"""Check Markdown links, embedded examples, and generated Schema sync."""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import tempfile
import unicodedata
from pathlib import Path

from quality_common import CRATE, ROOT, QUALITY_ROOT, fail, metadata, write_json


def local_links() -> list[dict[str, str]]:
    failures = []
    documents = [ROOT / "README.md", ROOT / "README.zh-CN.md", ROOT / "CONTRIBUTING.md", ROOT / "CHANGELOG.md", *ROOT.glob("docs/**/*.md")]
    pattern = re.compile(r"\]\(([^)\s]+)")
    for document in documents:
        source = document.read_text(errors="replace")
        headings = {
            anchor(heading): heading
            for heading in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", source, flags=re.MULTILINE)
        }
        headings.update({identifier: identifier for identifier in re.findall(r"<a\s+id=[\"']([^\"']+)[\"']", source)})
        for target in pattern.findall(source):
            if target.startswith(("http://", "https://", "mailto:")):
                continue
            path, fragment = (target.split("#", 1) + [""])[:2] if "#" in target else (target, "")
            linked = document if not path else document.parent / path
            if path and not linked.exists():
                failures.append({"document": str(document.relative_to(ROOT)), "target": target})
            elif fragment:
                if linked.resolve() == document.resolve():
                    linked_headings = headings
                elif linked.is_file():
                    linked_headings = {
                        anchor(heading): heading
                        for heading in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", linked.read_text(errors="replace"), flags=re.MULTILINE)
                    }
                    linked_headings.update(
                        {
                            identifier: identifier
                            for identifier in re.findall(r"<a\s+id=[\"']([^\"']+)[\"']", linked.read_text(errors="replace"))
                        }
                    )
                else:
                    linked_headings = {}
                if fragment not in linked_headings and anchor(fragment) not in linked_headings:
                    failures.append({"document": str(document.relative_to(ROOT)), "target": target})
    return failures


def anchor(value: str) -> str:
    value = unicodedata.normalize("NFKD", value).lower()
    value = re.sub(r"[^\w\s-]", "", value, flags=re.UNICODE)
    return re.sub(r"[-\s]+", "-", value).strip("-")


def run(output: Path) -> int:
    link_failures = local_links()
    examples = []
    for preset in sorted((CRATE / "presets").glob("*.flow.toml")):
        with tempfile.TemporaryDirectory(prefix="harness-gate-example-") as directory:
            root = Path(directory)
            preset_name = preset.name.removesuffix(".flow.toml")
            initialized = subprocess.run(
                ["cargo", "run", "--manifest-path", str(CRATE / "Cargo.toml"), "--locked", "--", "init", "--project-root", str(root), "--preset", preset_name],
                cwd=ROOT,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            checked = initialized.returncode == 0 and subprocess.run(
                ["cargo", "run", "--manifest-path", str(CRATE / "Cargo.toml"), "--locked", "--", "config", "check", "--project-root", str(root)],
                cwd=ROOT,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            ).returncode == 0
            examples.append({"path": str(preset.relative_to(ROOT)), "status": "pass" if checked else "fail"})
    migration_fixture = Path(__file__).with_name("fixtures") / "v1-flow.toml"
    with tempfile.TemporaryDirectory(prefix="harness-gate-migration-") as directory:
        root = Path(directory)
        shutil.copy2(migration_fixture, root / "legacy.flow.toml")
        (root / ".harness-gate").mkdir()
        migrated = root / ".harness-gate" / "flow.toml"
        migration = subprocess.run(
            [
                "cargo",
                "run",
                "--manifest-path",
                str(CRATE / "Cargo.toml"),
                "--locked",
                "--",
                "--project-root",
                str(root),
                "config",
                "migrate",
                "--input",
                "legacy.flow.toml",
                "--output",
                ".harness-gate/flow.toml",
            ],
            cwd=ROOT,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        (root / ".harness-gate" / "audit.toml").write_text(
            (CRATE / "presets" / "empty.audit.toml").read_text()
        )
        migration_checked = migration.returncode == 0 and subprocess.run(
            ["cargo", "run", "--manifest-path", str(CRATE / "Cargo.toml"), "--locked", "--", "config", "check", "--project-root", str(root)],
            cwd=ROOT,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        ).returncode == 0
        migration_checked = migration_checked and (root / ".harness-gate" / "secrets.toml").is_file()
    with tempfile.TemporaryDirectory(prefix="harness-gate-schema-") as directory:
        generated = Path(directory) / "flow.schema.json"
        schema = subprocess.run(["cargo", "run", "--manifest-path", str(CRATE / "Cargo.toml"), "--locked", "--", "schema", "export", "--output", str(generated)], cwd=ROOT, capture_output=True, text=True)
        committed = ROOT / "schema" / "flow.schema.json"
        schema_synced = schema.returncode == 0 and generated.read_bytes() == committed.read_bytes()
    machine_schema = ROOT / "schema" / "machine-result.schema.json"
    machine_schema_valid = False
    try:
        machine_data = json.loads(machine_schema.read_text())
        required = set(machine_data.get("required", []))
        machine_schema_valid = machine_data.get("properties", {}).get("schema_version", {}).get("const") == "1" and {
            "schema_version",
            "scope",
            "services",
            "steps",
            "skipped_steps",
            "warnings",
            "failures",
            "artifacts",
            "evidence_complete",
            "status",
        } <= required
    except (OSError, json.JSONDecodeError, TypeError):
        machine_schema_valid = False
    result = {**metadata(tool="docs-consistency"), "link_failures": link_failures, "examples": examples, "migration": {"path": str(migration_fixture.relative_to(ROOT)), "status": "pass" if migration_checked else "fail"}, "schema_synced": schema_synced, "machine_schema_valid": machine_schema_valid, "status": "pass" if not link_failures and schema_synced and machine_schema_valid and migration_checked and all(item["status"] == "pass" for item in examples) else "fail"}
    write_json(output, result)
    if result["status"] != "pass":
        fail("documentation, examples, or schema synchronization failed")
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=QUALITY_ROOT / "docs-consistency.json")
    args = parser.parse_args()
    raise SystemExit(run(args.output))
