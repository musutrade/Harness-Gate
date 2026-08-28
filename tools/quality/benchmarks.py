#!/usr/bin/env python3
"""Capture deterministic verification, test, and release-small baselines."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any

from quality_common import CRATE, QUALITY_ROOT, fail, metadata, sha256, stats, write_json


HARNESS_VERSION = 1
FIXTURE = Path(__file__).with_name("fixtures") / "benchmark"


def timed(command: list[str], cwd: Path) -> tuple[float, subprocess.CompletedProcess[str]]:
    started = time.perf_counter()
    result = subprocess.run(command, cwd=cwd, text=True, capture_output=True)
    return time.perf_counter() - started, result


def init_fixture(destination: Path) -> None:
    shutil.copytree(FIXTURE, destination, dirs_exist_ok=True)
    subprocess.run(["git", "init", "--quiet"], cwd=destination, check=True)
    subprocess.run(["git", "config", "user.name", "Quality Benchmark"], cwd=destination, check=True)
    subprocess.run(["git", "config", "user.email", "quality@example.invalid"], cwd=destination, check=True)
    subprocess.run(["git", "add", "."], cwd=destination, check=True)
    subprocess.run(["git", "commit", "--quiet", "-m", "fixture"], cwd=destination, check=True)


def verification_sample(binary: Path, root: Path, raw_root: Path, number: int) -> dict[str, Any]:
    reports = root / ".harness-gate" / "reports"
    shutil.rmtree(reports, ignore_errors=True)
    seconds, result = timed([str(binary), "--color", "never", "--project-root", str(root), "verify", "--all"], CRATE.parent.parent)
    if result.returncode != 0:
        fail(f"benchmark verification sample {number} failed: {result.stderr or result.stdout}")
    report_path = reports / "test_result.json"
    if not report_path.is_file():
        fail(f"benchmark verification sample {number} did not write {report_path}")
    report = json.loads(report_path.read_text())
    archive = raw_root / f"sample-{number}"
    shutil.copytree(reports, archive)
    steps = []
    for step in report["steps"]:
        source_log = Path(step["log"])
        try:
            relative_log = source_log.relative_to(reports)
        except ValueError:
            relative_log = Path("logs") / source_log.name
        steps.append(
            {
                "label": step["label"],
                "duration_ms": step["duration_ms"],
                "passed": step["passed"],
                "log": str(Path("benchmark-runs") / f"sample-{number}" / relative_log),
            }
        )
    if len(steps) < 4 or not report["passed"]:
        fail(f"benchmark verification sample {number} did not complete two gates and two configured steps")
    return {"sample": number, "seconds": seconds, "steps": steps, "report": str(Path("benchmark-runs") / f"sample-{number}" / "test_result.json")}


def run(output: Path, samples: int, raw_dir: Path | None = None) -> int:
    if samples < 1:
        fail("at least one warm sample is required")
    subprocess.run(["cargo", "build", "--manifest-path", str(CRATE / "Cargo.toml"), "--locked", "--profile", "release-small"], check=True, cwd=CRATE.parent.parent)
    binary = CRATE / "target" / "release-small" / ("harness-gate.exe" if os.name == "nt" else "harness-gate")
    if not binary.is_file():
        fail(f"release-small binary missing: {binary}")

    cold_seconds, cold_result = timed(["cargo", "nextest", "run", "--manifest-path", str(CRATE / "Cargo.toml"), "--locked"], CRATE.parent.parent)
    if cold_result.returncode != 0:
        fail("cold nextest sample failed")
    warm_seconds = []
    for _ in range(samples):
        seconds, result = timed(["cargo", "nextest", "run", "--manifest-path", str(CRATE / "Cargo.toml"), "--locked"], CRATE.parent.parent)
        if result.returncode != 0:
            fail("warm nextest sample failed")
        warm_seconds.append(seconds)

    raw_root = raw_dir or output.parent / "benchmark-runs"
    shutil.rmtree(raw_root, ignore_errors=True)
    raw_root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="harness-gate-quality-benchmark-") as directory:
        root = Path(directory)
        init_fixture(root)
        verification = [verification_sample(binary, root, raw_root, index + 1) for index in range(samples)]
    verification_seconds = [sample["seconds"] for sample in verification]
    result = {
        **metadata(tool="quality-benchmarks", harness_version=HARNESS_VERSION, fixture_version=1, cache_state="cold-and-warm", raw_evidence=str(raw_root)),
        "verification_seconds": {"samples": verification_seconds, **stats(verification_seconds)},
        "verification_samples": verification,
        "tests_seconds": {"cold": cold_seconds, "warm_samples": warm_seconds, **stats(warm_seconds)},
        "binary": {"path": str(binary), "bytes": binary.stat().st_size, "sha256": sha256(binary)},
    }
    write_json(output, result)
    output.with_suffix(".md").write_text(
        "# Quality Baseline\n\n"
        f"Verification median: **{result['verification_seconds']['median']:.3f}s**\n\n"
        f"Test warm median: **{result['tests_seconds']['median']:.3f}s** (cold: {cold_seconds:.3f}s)\n\n"
        f"Release-small binary: **{result['binary']['bytes']} bytes**\n"
        f"SHA-256: `{result['binary']['sha256']}`\n"
    )
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=QUALITY_ROOT / "benchmarks.json")
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--raw-dir", type=Path, help="dedicated directory for retained per-sample reports")
    args = parser.parse_args()
    raise SystemExit(run(args.output, args.samples, args.raw_dir))
