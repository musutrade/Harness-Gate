#!/usr/bin/env python3
"""Validate the critical-path inventory against a test evidence manifest."""

from __future__ import annotations

import argparse
import json
import sys
import tomllib
from pathlib import Path

from quality_common import QUALITY_ROOT, fail, metadata, write_json


def executed_tests(evidence: Path) -> set[str]:
    if not evidence.exists():
        return set()
    if evidence.suffix == ".jsonl":
        names = set()
        for line in evidence.read_text().splitlines():
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if event.get("type") == "test" and event.get("event") in {"ok", "passed"}:
                name = str(event.get("name", ""))
                if "$" in name:
                    target, normalized = name.split("$", 1)
                    target = target.removeprefix("harness-gate::")
                    if "::" not in normalized:
                        normalized = f"{target}::{normalized}"
                else:
                    normalized = name
                normalized = normalized.replace("$", "::")
                for prefix in ("harness-gate::bin/harness-gate::", "harness-gate::"):
                    if normalized.startswith(prefix):
                        normalized = normalized[len(prefix) :]
                        break
                names.add(normalized)
        return names
    return set(evidence.read_text().splitlines())


def run(output: Path, evidence: Path, threshold: float, coverage: Path | None = None) -> int:
    inventory = tomllib.loads((Path(__file__).with_name("critical_paths.toml")).read_text())
    executed = executed_tests(evidence)
    covered_modules: dict[str, dict] = {}
    if coverage and coverage.exists():
        covered_modules = json.loads(coverage.read_text()).get("modules", {})
    rows = []
    failures = []
    platform = "windows" if sys.platform.startswith("win") else "macos" if sys.platform == "darwin" else "linux"
    for path in inventory["paths"]:
        applicable = platform in path.get("platforms", [platform])
        test_name = path["test"].replace("::", "::")
        status = "pass" if test_name in executed or path["test"] in executed else "fail"
        module_covered = not coverage or covered_modules.get(path["module"], {}).get("status") in {"pass", "informational"}
        traceable = applicable and status == "pass" and module_covered
        rows.append({**path, "applicable": applicable, "status": (status if module_covered else "fail") if applicable else "not-applicable", "traceable": traceable, "evidence": str(evidence)})
        if applicable and not traceable:
            failures.append(path["id"])
    total = sum(row["applicable"] for row in rows)
    passed = sum(1 for row in rows if row["applicable"] and row["traceable"])
    percent = passed / total * 100 if total else 0
    result = {
        **metadata(tool="critical-path-matrix", threshold=threshold),
        "rows": rows,
        "summary": {"platform": platform, "passed": passed, "total": total, "percent": percent, "status": "pass" if percent >= threshold and not {"process.cancellation", "process.tree_cleanup"}.intersection(failures) else "fail"},
    }
    write_json(output, result)
    lines = ["# Critical Path Evidence", "", f"Platform: `{platform}`", "", "| ID | Test | Applicable | Status |", "| --- | --- | --- | --- |"]
    lines.extend(f"| `{row['id']}` | `{row['test']}` | {row['applicable']} | {row['status']} |" for row in rows)
    lines.extend(["", f"Evidence: **{percent:.2f}%** ({passed}/{total})", ""])
    output.with_suffix(".md").write_text("\n".join(lines))
    if result["summary"]["status"] != "pass":
        fail(f"critical path evidence below {threshold:.1f}% or mandatory rows missing: {', '.join(failures)}")
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=QUALITY_ROOT / "critical-paths.json")
    parser.add_argument("--evidence", type=Path, default=QUALITY_ROOT / "tests.txt")
    parser.add_argument("--coverage", type=Path)
    parser.add_argument("--threshold", type=float, default=95.0)
    args = parser.parse_args()
    raise SystemExit(run(args.output, args.evidence, args.threshold, args.coverage))
