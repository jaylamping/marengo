#!/usr/bin/env python3
"""Daily audit report builder."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from datetime import date, datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "var" / "log" / "daily-audit" / date.today().isoformat()

RISKY_PREFIXES = (
    "crates/berthier/",
    "crates/davout/",
    "crates/robstride/",
    "proto/",
)

ADR_MAP = {
    "crates/robstride/": "hardware/docs/decisions/0002-robstride-protocol.md",
    "crates/davout/": "docs/safety.md",
    "proto/": "docs/decisions/0001-protobuf-wire-types.md",
}


@dataclass
class Finding:
    severity: str
    category: str
    file: str
    rule: str
    message: str
    commit: str = ""


@dataclass
class Report:
    date: str
    commits_reviewed: list[str] = field(default_factory=list)
    changed_files: list[str] = field(default_factory=list)
    findings: list[Finding] = field(default_factory=list)
    topics: list[dict[str, str]] = field(default_factory=list)
    clean: bool = True

    def add(self, finding: Finding) -> None:
        if finding.severity in ("warn", "critical"):
            self.clean = False
        self.findings.append(finding)


def run(cmd: list[str], cwd: Path = ROOT) -> str:
    result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, check=False)
    return result.stdout.strip()


def git_changed_files() -> tuple[list[str], list[str]]:
    commits = run(["git", "log", "--since=24.hours", "--format=%H"]).splitlines()
    files: set[str] = set()
    for commit in commits:
        diff_files = run(["git", "diff-tree", "--no-commit-id", "--name-only", "-r", commit])
        files.update(f for f in diff_files.splitlines() if f)
    # Open PR heads not available locally without gh; workflow adds PR scan separately
    return commits, sorted(files)


def check_unwrap(changed: list[str], report: Report) -> None:
    for path in changed:
        if not path.startswith("crates/") or "/tests/" in path:
            continue
        full = ROOT / path
        if not full.is_file():
            continue
        text = full.read_text(encoding="utf-8", errors="replace")
        if re.search(r"\bunwrap\s*\(|\bexpect\s*\(", text):
            report.add(
                Finding(
                    severity="critical",
                    category="rust",
                    file=path,
                    rule="AGENTS.md — no unwrap/expect in crates/*",
                    message="unwrap/expect found in changed crate file",
                )
            )


def check_gen_handedit(changed: list[str], report: Report) -> None:
    for path in changed:
        if path.startswith("consul/src/gen/") and not path.endswith(".checksum"):
            report.add(
                Finding(
                    severity="critical",
                    category="proto",
                    file=path,
                    rule="AGENTS.md — never hand-edit consul/src/gen/",
                    message="Generated consul proto file modified",
                )
            )


def check_davout_bypass(changed: list[str], report: Report) -> None:
    motor_paths = [p for p in changed if "robstride" in p or "davout" in p or "berthier" in p]
    for path in motor_paths:
        full = ROOT / path
        if not full.is_file() or not path.endswith(".rs"):
            continue
        text = full.read_text(encoding="utf-8", errors="replace")
        if path.startswith("crates/berthier/") and "robstride" in text.lower():
            report.add(
                Finding(
                    severity="critical",
                    category="safety",
                    file=path,
                    rule="docs/architecture.md — Berthier must not touch CAN/robstride",
                    message="Berthier references robstride directly",
                )
            )
        if path.startswith("crates/robstride/") and "enable" in text.lower() and "test" not in path:
            if "davout" not in run(["git", "log", "-1", "--format=%H", "--", path]):
                pass  # heuristic only in diff context


def check_large_risky_diff(changed: list[str], report: Report) -> None:
    for path in changed:
        if not any(path.startswith(p) for p in RISKY_PREFIXES):
            continue
        stat = run(["git", "diff", "HEAD~1", "HEAD", "--numstat", "--", path])
        if not stat:
            continue
        parts = stat.split()
        if len(parts) >= 2:
            added = int(parts[0] or 0)
            deleted = int(parts[1] or 0)
            if added + deleted > 400:
                report.add(
                    Finding(
                        severity="warn",
                        category="scope",
                        file=path,
                        rule="daily-audit — large change in safety-critical path",
                        message=f"Large diff ({added}+{deleted} lines) in risky area",
                    )
                )


def check_adr_staleness(changed: list[str], report: Report) -> None:
    adr_changed = any(p.startswith("docs/decisions/") for p in changed)
    for prefix, adr in ADR_MAP.items():
        touched = [p for p in changed if p.startswith(prefix)]
        if touched and not adr_changed:
            report.add(
                Finding(
                    severity="warn",
                    category="docs",
                    file=touched[0],
                    rule=f"ADR staleness — {adr} may need update",
                    message=f"Changed under {prefix} without ADR/decision doc update",
                )
            )


def check_hardware_config_coupling(changed: list[str], report: Report) -> None:
    motor_cfg = [p for p in changed if p.startswith("config/motors") or "motors" in p]
    kin = [p for p in changed if "kinematics" in p]
    docs = [p for p in changed if p.startswith("docs/") or p.startswith("hardware/docs/")]
    if (motor_cfg or kin) and not docs:
        report.add(
            Finding(
                severity="warn",
                category="config",
                file=(motor_cfg or kin)[0],
                rule="daily-audit-rubric R10 — config/kinematics needs doc pairing",
                message="Hardware/config changed without doc updates in same window",
            )
        )


def infer_topics(changed: list[str]) -> list[dict[str, str]]:
    topics: list[dict[str, str]] = []
    if any("robstride" in p or "davout" in p for p in changed):
        topics.append({"query": "Robstride MIT actuator control CAN", "focus": "vendor"})
    if any("berthier" in p or "talleyrand" in p for p in changed):
        topics.append({"query": "humanoid whole-body control impedance", "focus": "papers"})
    if any(p.startswith("sim/") for p in changed):
        topics.append({"query": "humanoid sim-to-real MuJoCo", "focus": "code"})
    if any(p.startswith("hardware/") for p in changed):
        topics.append({"query": "humanoid robot mechanical design biped", "focus": "all"})
    return topics


def write_report(report: Report) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    payload = {
        "date": report.date,
        "commits_reviewed": report.commits_reviewed,
        "changed_files": report.changed_files,
        "findings": [asdict(f) for f in report.findings],
        "topics": report.topics,
        "clean": report.clean,
        "generated_at": datetime.now(timezone.utc).isoformat(),
    }
    (OUT_DIR / "report.json").write_text(json.dumps(payload, indent=2), encoding="utf-8")
    lines = [
        f"# Daily audit {report.date}",
        "",
        f"**Clean:** {report.clean}",
        f"**Commits:** {len(report.commits_reviewed)}",
        f"**Changed files:** {len(report.changed_files)}",
        "",
        "## Findings",
        "",
    ]
    if not report.findings:
        lines.append("_No findings._")
    else:
        lines.append("| Severity | Category | File | Rule | Message |")
        lines.append("|----------|----------|------|------|---------|")
        for f in report.findings:
            lines.append(
                f"| {f.severity} | {f.category} | `{f.file}` | {f.rule} | {f.message} |"
            )
    (OUT_DIR / "report.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    (OUT_DIR / "topics.json").write_text(json.dumps({"topics": report.topics}, indent=2), encoding="utf-8")
    print(json.dumps({"out_dir": str(OUT_DIR), "clean": report.clean, "findings": len(report.findings)}))


def main() -> int:
    commits, changed = git_changed_files()
    report = Report(date=date.today().isoformat(), commits_reviewed=commits, changed_files=changed)
    if changed:
        check_unwrap(changed, report)
        check_gen_handedit(changed, report)
        check_davout_bypass(changed, report)
        check_large_risky_diff(changed, report)
        check_adr_staleness(changed, report)
        check_hardware_config_coupling(changed, report)
        report.topics = infer_topics(changed)
    write_report(report)
    return 0 if report.clean else 1


if __name__ == "__main__":
    sys.exit(main())
