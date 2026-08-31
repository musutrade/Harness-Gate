#!/usr/bin/env python3
"""Exercise the public CLI contract and compare normalized golden snapshots."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import tempfile
from pathlib import Path
from typing import Any

from quality_common import CRATE, QUALITY_ROOT, fail, metadata, write_json


SNAPSHOT = Path(__file__).with_name("snapshots") / "contracts.json"


def normalize(value: str, root: Path) -> str:
    value = value.replace(str(root), "<PROJECT_ROOT>")
    value = re.sub(r"inv-\d+-\d{9}-\d+-\d+", "<INVOCATION_ID>", value)
    value = re.sub(r"\b20\d{2}-\d{2}-\d{2}T[^\s]+", "<TIMESTAMP>", value)
    value = re.sub(r"\b\d+ ms\b", "<DURATION>", value)
    value = re.sub(r"\b\d+\.\d+s\b", "<DURATION>", value)
    value = re.sub(r'("duration_ms"\s*:\s*)\d+', r'\1"<DURATION_MS>"', value)
    return value


def command(binary: Path, root: Path, args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(binary), "--color", "never", "--project-root", str(root), *args],
        cwd=CRATE.parent.parent,
        text=True,
        capture_output=True,
        env={**os.environ, "LC_ALL": "C", "TZ": "UTC"},
    )


def init(root: Path, binary: Path, preset: str = "generic", git: bool = False) -> None:
    result = command(binary, root, ["init", "--preset", preset])
    if result.returncode != 0:
        raise RuntimeError(result.stderr or result.stdout)
    if git:
        subprocess.run(["git", "init", "--quiet"], cwd=root, check=True)
        subprocess.run(["git", "config", "user.name", "Quality Test"], cwd=root, check=True)
        subprocess.run(["git", "config", "user.email", "quality@example.invalid"], cwd=root, check=True)
        subprocess.run(["git", "add", "."], cwd=root, check=True)
        subprocess.run(["git", "commit", "--quiet", "-m", "fixture"], cwd=root, check=True)


def report_snapshot(root: Path, names: list[str]) -> dict[str, Any]:
    report_root = root / ".harness-gate" / "reports"
    snapshot: dict[str, Any] = {}
    for name in names:
        path = report_root / name
        snapshot[name] = {
            "exists": path.is_file(),
            "content": normalize(path.read_text(errors="replace"), root) if path.is_file() else None,
        }
    logs = report_root / "logs"
    if logs.is_dir():
        snapshot["logs"] = {
            str(path.relative_to(report_root)): normalize(path.read_text(errors="replace"), root)
            for path in sorted(logs.glob("*"))
            if path.is_file()
        }
    return snapshot


def scenario(
    binary: Path,
    name: str,
    args: list[str],
    expected: int,
    setup: str = "none",
    report_names: list[str] | None = None,
    expected_order: list[str] | None = None,
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix=f"harness-gate-contract-{name}-") as directory:
        root = Path(directory)
        if setup == "init":
            init(root, binary)
        elif setup == "git":
            init(root, binary, git=True)
        elif setup == "invalid":
            init(root, binary)
            (root / ".harness-gate" / "flow.toml").write_text("version = 2\ninvalid = true\n")
        elif setup == "verify-failure":
            init(root, binary, git=True)
            flow = root / ".harness-gate" / "flow.toml"
            flow.write_text(
                re.sub(
                    r'(\[\[steps\]\]\nid = "project\.diff-check".*?\nprogram = )"git"',
                    r'\1"false"',
                    flow.read_text(),
                    count=1,
                    flags=re.DOTALL,
                )
            )
        elif setup == "verify-parallel":
            init(root, binary, git=True)
            flow = root / ".harness-gate" / "flow.toml"
            source = flow.read_text()
            source = source.replace('profiles = ["hook"]', 'profiles = ["full", "hook"]')
            source = source.replace(
                "[execution]\nparallel = false\n",
                "[execution]\nparallel = true\nmax_parallel = 2\n",
            )
            flow.write_text(source)
        result = command(binary, root, args)
        stdout = normalize(result.stdout, root)
        stderr = normalize(result.stderr, root)
        combined = stdout + stderr
        error_codes = sorted(set(re.findall(r"ERROR \[(E\d{4})\]", combined)))
        selected_reports = report_names or []
        publication_order: list[str] | None = None
        report_path = root / ".harness-gate" / "reports" / "test_result.json"
        if report_path.is_file() and expected_order is not None:
            report = json.loads(report_path.read_text())
            publication_order = [step["label"] for step in report.get("steps", [])]
            if publication_order != expected_order:
                result = subprocess.CompletedProcess(
                    result.args,
                    1,
                    result.stdout,
                    result.stderr,
                )
        record = {
            "name": name,
            "args": args,
            "expected_exit_code": expected,
            "exit_code": result.returncode,
            "stdout": stdout,
            "stderr": stderr,
            "error_codes": error_codes,
            "ansi": "\x1b[" in combined,
            "reports": report_snapshot(root, selected_reports),
            "status": "pass" if result.returncode == expected else "fail",
        }
        if expected_order is not None:
            record["publication_order"] = publication_order
        return record


def collect(binary: Path, staged_secrets: bool = True) -> list[dict[str, Any]]:
    return [
        scenario(binary, "help", ["--help"], 0),
        scenario(binary, "version", ["--version"], 0),
        scenario(binary, "presets", ["presets"], 0),
        scenario(binary, "init", ["init", "--preset", "generic"], 0),
        scenario(binary, "schema", ["schema", "export", "--output", "schema/flow.schema.json"], 0),
        scenario(binary, "config-valid", ["config", "check"], 0, "init"),
        scenario(binary, "config-json-valid", ["config", "check", "--format", "json"], 0, "init"),
        scenario(binary, "config-invalid", ["config", "check"], 1, "invalid"),
        scenario(binary, "config-json-invalid", ["config", "check", "--format", "json"], 1, "invalid"),
        scenario(binary, "scope", ["scope", "--all"], 0, "git", ["scope.json"]),
        scenario(binary, "secrets", ["secrets", *( ["--staged"] if staged_secrets else [] )], 0, "git", ["secret_scan.json"]),
        scenario(binary, "audit", ["audit"], 0, "init", ["review_context.json", "review_context.md"]),
        scenario(
            binary,
            "verify-success",
            ["verify", "--all"],
            0,
            "git",
            ["test_result.json", "test_result.md"],
            ["secret scan", "architecture audit", "Git whitespace check"],
        ),
        scenario(
            binary,
            "verify-parallel-success",
            ["verify", "--all"],
            0,
            "verify-parallel",
            ["test_result.json", "test_result.md"],
            ["secret scan", "architecture audit", "Git whitespace check", "staged Git whitespace check"],
        ),
        scenario(binary, "verify-failure", ["verify", "--all"], 1, "verify-failure", ["test_result.json", "test_result.md"]),
        scenario(binary, "verify-one-step", ["step", "project.diff-check"], 0, "git", ["test_result.json", "test_result.md"]),
        scenario(binary, "unknown-profile", ["verify", "--all", "--profile", "missing"], 1, "init"),
        scenario(binary, "unknown-step", ["step", "missing.step"], 1, "init"),
        scenario(binary, "unknown-command", ["missing-command"], 2),
    ]


def run(output: Path, accept: bool = False, structured: bool = False) -> int:
    binary = CRATE / "target" / "debug" / ("harness-gate.exe" if os.name == "nt" else "harness-gate")
    if not binary.exists():
        subprocess.run(["cargo", "build", "--manifest-path", str(CRATE / "Cargo.toml"), "--locked"], check=True, cwd=CRATE.parent.parent)
    scenarios = collect(binary, staged_secrets=not structured)
    report = {**metadata(tool="cli-contracts", snapshot_version=1), "scenarios": scenarios}
    write_json(output, report)
    if accept:
        SNAPSHOT.parent.mkdir(parents=True, exist_ok=True)
        write_json(SNAPSHOT, {"snapshot_version": 1, "scenarios": scenarios})
    elif not structured and SNAPSHOT.exists():
        expected = json.loads(SNAPSHOT.read_text())
        actual = {"snapshot_version": 1, "scenarios": scenarios}
        if actual != expected:
            fail(f"CLI contract snapshot differs; review {SNAPSHOT} and rerun with --accept")
    failures = [item["name"] for item in scenarios if item["status"] != "pass" or item["ansi"]]
    if failures:
        fail(f"contract scenarios failed: {', '.join(failures)}")
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=QUALITY_ROOT / "contracts.json")
    parser.add_argument("--accept", action="store_true", help="write a reviewed snapshot; never use in CI")
    parser.add_argument("--structured", action="store_true", help="assert structured contracts without a Linux textual snapshot")
    args = parser.parse_args()
    raise SystemExit(run(args.output, args.accept, args.structured))
