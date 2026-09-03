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
    documents = [
        ROOT / "README.md",
        ROOT / "README.zh-CN.md",
        ROOT / "CONTRIBUTING.md",
        ROOT / "CODE_OF_CONDUCT.md",
        ROOT / "CHANGELOG.md",
        ROOT / "SECURITY.md",
        *ROOT.glob("docs/**/*.md"),
        *ROOT.glob("schema/*.md"),
    ]
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


SANDBOX_NEGATION_MARKERS = (
    "not",
    "no ",
    "never",
    "without",
    "deferred",
    "future",
    "separate",
    "rejected",
    "must not",
    "should not",
    "cannot",
    "can't",
    "isn't",
    "aren't",
    "does not",
    "do not",
    "not an",
    "unless",
    "until",
    "before",
    "out of scope",
    "non-goal",
    "would overstate",
    "later",
    "may",
    "might",
    "could",
)

SANDBOX_CLAIM_PATTERNS = (
    re.compile(
        r"\b(?:runs?|executes?|enforces?|provides?|offers?|guarantees?|gives?|uses?|claims?)\b"
        r"[^.!?]{0,120}\b(?:operating-system|os[-\s]?enforced|os[-\s]?level|kernel)?"
        r"[-\s]?(?:network|filesystem|resource|process)?[-\s]?sandbox(?:ed|ing)?\b",
        re.IGNORECASE,
    ),
    re.compile(
        r"\b(?:complete|full)\b[^.!?]{0,60}\bdescendant\s+(?:isolation|containment)\b",
        re.IGNORECASE,
    ),
    re.compile(
        r"\b(?:os|operating-system)[-\s]?(?:level|enforced)?[-\s]?"
        r"(?:network|filesystem|resource|process)\s+isolation\b",
        re.IGNORECASE,
    ),
)


def unsupported_sandbox_claims(text: str) -> list[str]:
    """Return positive OS-sandbox or complete-descendant-isolation claims.

    Sentences that deny, defer, reject, or bound such isolation are accepted;
    only unsupported positive claims are reported so the R-07 wording check
    fails closed when documentation drifts toward a stronger promise.
    """
    compact = re.sub(r"\s+", " ", text)
    failures: list[str] = []
    for pattern in SANDBOX_CLAIM_PATTERNS:
        for match in pattern.finditer(compact):
            context = compact[max(0, match.start() - 160) : match.end() + 80]
            lowered = context.lower()
            if any(marker in lowered for marker in SANDBOX_NEGATION_MARKERS):
                continue
            failures.append(context.strip())
    return failures


def sandbox_wording_failures() -> list[dict[str, str]]:
    documents = [
        ROOT / "README.md",
        ROOT / "README.zh-CN.md",
        ROOT / "CONTRIBUTING.md",
        ROOT / "CODE_OF_CONDUCT.md",
        ROOT / "SECURITY.md",
        *ROOT.glob("docs/**/*.md"),
        *ROOT.glob("schema/*.md"),
        *ROOT.glob("openspec/changes/*/*.md"),
        *ROOT.glob("openspec/changes/*/specs/**/*.md"),
        *ROOT.glob("tools/harness-gate/src/**/*.rs"),
    ]
    failures = []
    for document in documents:
        try:
            source = document.read_text(errors="replace")
        except OSError:
            continue
        for claim in unsupported_sandbox_claims(source):
            failures.append(
                {
                    "document": str(document.relative_to(ROOT)),
                    "claim": claim,
                }
            )
    return failures


def anchor(value: str) -> str:
    value = unicodedata.normalize("NFKD", value).lower()
    value = re.sub(r"[^\w\s-]", "", value, flags=re.UNICODE)
    return re.sub(r"[-\s]+", "-", value).strip("-")


def run(output: Path) -> int:
    link_failures = local_links()
    sandbox_failures = sandbox_wording_failures()
    english_config = ROOT / "docs" / "configuration.md"
    chinese_config = ROOT / "docs" / "configuration.zh-CN.md"
    schema_catalog = ROOT / "schema" / "README.md"
    language_docs_valid = (
        english_config.is_file()
        and chinese_config.is_file()
        and schema_catalog.is_file()
        and "# harness-gate schema v2 configuration reference" in english_config.read_text(errors="replace").lower()
    )
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
        schema_root = Path(directory)
        generated = schema_root / "flow.schema.json"
        schema = subprocess.run(
            [
                "cargo",
                "run",
                "--manifest-path",
                str(CRATE / "Cargo.toml"),
                "--locked",
                "--",
                "--project-root",
                str(schema_root),
                "schema",
                "export",
                "--output",
                "flow.schema.json",
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        committed = ROOT / "schema" / "flow.schema.json"
        schema_synced = schema.returncode == 0 and generated.read_bytes() == committed.read_bytes()
    machine_schema = ROOT / "schema" / "machine-result.schema.json"
    machine_schema_valid = False
    try:
        machine_data = json.loads(machine_schema.read_text())
        required = set(machine_data.get("required", []))
        machine_schema_valid = machine_data.get("properties", {}).get("schema_version", {}).get("const") == "1" and {
            "schema_version",
            "input_mode",
            "project_identity",
            "source_identity",
            "execution_root",
            "configuration_digest",
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
    manifest_schema = ROOT / "schema" / "artifact-manifest.schema.json"
    manifest_schema_valid = False
    try:
        manifest_data = json.loads(manifest_schema.read_text())
        manifest_required = set(manifest_data.get("required", []))
        artifact_required = set(
            manifest_data.get("definitions", {}).get("artifact", {}).get("required", [])
        )
        manifest_schema_valid = (
            manifest_data.get("properties", {}).get("schema_version", {}).get("const") == "1"
            and {"schema_version", "invocation_id", "generated_at", "artifacts"} <= manifest_required
            and {"path", "kind", "size_bytes", "sha256"} <= artifact_required
        )
    except (OSError, json.JSONDecodeError, TypeError):
        manifest_schema_valid = False
    registry_schema = ROOT / "schema" / "artifact-registry.schema.json"
    registry_schema_valid = False
    try:
        registry_data = json.loads(registry_schema.read_text())
        registry_required = set(registry_data.get("required", []))
        registry_artifact_required = set(
            registry_data.get("definitions", {}).get("artifact", {}).get("required", [])
        )
        registry_schema_valid = (
            registry_data.get("properties", {}).get("schema_version", {}).get("const") == "1"
            and {"schema_version", "invocation_id", "artifacts"} <= registry_required
            and {"invocation_id", "path", "kind", "size_bytes", "sha256"}
            <= registry_artifact_required
        )
    except (OSError, json.JSONDecodeError, TypeError):
        registry_schema_valid = False
    sandbox_wording = {
        "failures": sandbox_failures,
        "status": "fail" if sandbox_failures else "pass",
    }
    result = {**metadata(tool="docs-consistency"), "link_failures": link_failures, "examples": examples, "migration": {"path": str(migration_fixture.relative_to(ROOT)), "status": "pass" if migration_checked else "fail"}, "language_docs_valid": language_docs_valid, "schema_synced": schema_synced, "machine_schema_valid": machine_schema_valid, "manifest_schema_valid": manifest_schema_valid, "registry_schema_valid": registry_schema_valid, "sandbox_wording": sandbox_wording, "status": "pass" if not link_failures and language_docs_valid and schema_synced and machine_schema_valid and manifest_schema_valid and registry_schema_valid and migration_checked and all(item["status"] == "pass" for item in examples) and sandbox_wording["status"] == "pass" else "fail"}
    write_json(output, result)
    if result["status"] != "pass":
        if sandbox_failures:
            fail("unsupported OS-sandbox or descendant-isolation wording found")
        fail("documentation, examples, or schema synchronization failed")
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=QUALITY_ROOT / "docs-consistency.json")
    args = parser.parse_args()
    raise SystemExit(run(args.output))
