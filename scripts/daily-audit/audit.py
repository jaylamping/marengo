#!/usr/bin/env python3
"""Daily audit report builder."""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

RISKY_PREFIXES = (
    "crates/berthier/",
    "crates/davout/",
    "crates/robstride/",
    "proto/",
)

SAFETY_PREFIXES = RISKY_PREFIXES + (
    "config/",
    "crates/marengo-config/",
)

@dataclass(frozen=True)
class AdrRule:
    """Path-pairing rule: source edits under ``prefix`` with matching suffixes need ``adr``."""

    prefix: str
    adr: str
    source_suffixes: tuple[str, ...] = (".rs",)


ADR_RULES: tuple[AdrRule, ...] = (
    AdrRule("crates/robstride/", "hardware/docs/decisions/0002-robstride-protocol.md"),
    AdrRule("crates/davout/", "docs/safety.md"),
    AdrRule("crates/berthier/", "docs/decisions/0007-bench-position-trajectory-control.md"),
    AdrRule("proto/", "docs/decisions/0001-protobuf-wire-types.md", (".proto",)),
)

# prefix → ADR path (rubric / callers that only need the mapping)
ADR_MAP = {rule.prefix: rule.adr for rule in ADR_RULES}

UNWRAP_RE = re.compile(r"\.unwrap\s*\(|\.expect\s*\(")
COMMENT_LINE_RE = re.compile(r"^\s*//")


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
    scan_windows: dict[str, str] = field(default_factory=dict)

    def add(self, finding: Finding) -> None:
        if finding.severity in ("warn", "critical"):
            self.clean = False
        self.findings.append(finding)


def utc_today() -> str:
    return datetime.now(timezone.utc).date().isoformat()


def out_dir_for(date_str: str | None = None) -> Path:
    return ROOT / "var" / "log" / "daily-audit" / (date_str or utc_today())


def run(cmd: list[str], cwd: Path = ROOT) -> str:
    result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, check=False)
    return result.stdout.strip()


def run_json(cmd: list[str], cwd: Path = ROOT) -> list | dict | None:
    proc = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, check=False)
    if proc.returncode != 0 or not proc.stdout.strip():
        return None
    return json.loads(proc.stdout)


def git_log_since(since: str) -> list[str]:
    return [c for c in run(["git", "log", f"--since={since}", "--format=%H"]).splitlines() if c]


def files_for_commits(commits: list[str]) -> set[str]:
    files: set[str] = set()
    for commit in commits:
        diff_files = run(["git", "diff-tree", "--no-commit-id", "--name-only", "-r", commit])
        files.update(f for f in diff_files.splitlines() if f)
    return files


def git_changed_files() -> tuple[list[str], list[str], dict[str, str]]:
    """Return commits, merged changed files, and scan window metadata."""
    commits_24h = git_log_since("24.hours")
    commits_7d = git_log_since("7.days")
    files_24h = files_for_commits(commits_24h)
    files_7d = files_for_commits(commits_7d)

    safety_from_7d = {f for f in files_7d if any(f.startswith(p) for p in SAFETY_PREFIXES)}
    merged = sorted(files_24h | safety_from_7d)
    commits = commits_7d

    windows = {
        "general": "24.hours",
        "safety_paths": "7.days",
        "files_from_24h": str(len(files_24h)),
        "files_from_7d_safety": str(len(safety_from_7d)),
    }
    return commits, merged, windows


def strip_rust_tests(source: str) -> str:
    """Drop #[cfg(test)] modules so test unwrap/expect do not false-positive."""
    lines = source.splitlines()
    out: list[str] = []
    i = 0
    while i < len(lines):
        if lines[i].strip() == "#[cfg(test)]":
            i += 1
            while i < len(lines) and not lines[i].strip().startswith("mod "):
                i += 1
            if i >= len(lines):
                break
            brace_depth = 0
            started = False
            while i < len(lines):
                line = lines[i]
                for ch in line:
                    if ch == "{":
                        brace_depth += 1
                        started = True
                    elif ch == "}":
                        brace_depth -= 1
                i += 1
                if started and brace_depth <= 0:
                    break
            continue
        out.append(lines[i])
        i += 1
    return "\n".join(out)


def production_rust_lines(source: str) -> str:
    body = strip_rust_tests(source)
    return "\n".join(line for line in body.splitlines() if not COMMENT_LINE_RE.match(line))


def check_unwrap(changed: list[str], report: Report) -> None:
    for path in changed:
        if not path.startswith("crates/") or "/tests/" in path:
            continue
        full = ROOT / path
        if not full.is_file() or not path.endswith(".rs"):
            continue
        text = production_rust_lines(full.read_text(encoding="utf-8", errors="replace"))
        if UNWRAP_RE.search(text):
            report.add(
                Finding(
                    severity="critical",
                    category="rust",
                    file=path,
                    rule="AGENTS.md R1 — no unwrap/expect in crates/* library code",
                    message="unwrap/expect found outside test modules",
                )
            )


def check_proto_checksum(changed: list[str], report: Report) -> None:
    proto_changed = any(path.startswith("proto/") for path in changed)
    checksum_changed = "consul/src/gen/.checksum" in changed
    if proto_changed and not checksum_changed:
        report.add(
            Finding(
                severity="warn",
                category="proto",
                file="consul/src/gen/.checksum",
                rule="R4 — proto change should refresh consul gen checksum",
                message="proto/ changed but consul/src/gen/.checksum not updated in scan window",
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
                    rule="AGENTS.md R3 — never hand-edit consul/src/gen/",
                    message="Generated consul proto file modified",
                )
            )


def check_davout_bypass(changed: list[str], report: Report) -> None:
    for path in changed:
        if not path.startswith("crates/berthier/") or not path.endswith(".rs"):
            continue
        full = ROOT / path
        if not full.is_file():
            continue
        for line in full.read_text(encoding="utf-8", errors="replace").splitlines():
            if COMMENT_LINE_RE.match(line):
                continue
            lower = line.lower()
            if "robstride" in lower or "use robstride" in lower:
                report.add(
                    Finding(
                        severity="critical",
                        category="safety",
                        file=path,
                        rule="docs/architecture.md R6 — Berthier must not touch CAN/robstride",
                        message="Non-comment Berthier reference to robstride/CAN layer",
                    )
                )
                break


def diff_line_count(path: str, since: str) -> tuple[int, int]:
    stat = run(["git", "log", f"--since={since}", "--format=%H", "--", path]).splitlines()
    if not stat:
        return 0, 0
    oldest = stat[-1]
    numstat = run(["git", "diff", f"{oldest}^", "HEAD", "--numstat", "--", path])
    added = deleted = 0
    for line in numstat.splitlines():
        parts = line.split()
        if len(parts) >= 2:
            added += int(parts[0] or 0)
            deleted += int(parts[1] or 0)
    return added, deleted


def _sources_for_rule(rule: AdrRule, changed: list[str]) -> list[str]:
    return [
        path
        for path in changed
        if path.startswith(rule.prefix) and path.endswith(rule.source_suffixes)
    ]


def paired_adr_updated(path: str, changed: list[str]) -> bool:
    changed_set = set(changed)
    return any(path.startswith(rule.prefix) and rule.adr in changed_set for rule in ADR_RULES)


def check_large_risky_diff(changed: list[str], report: Report) -> None:
    for path in changed:
        if not any(path.startswith(p) for p in RISKY_PREFIXES):
            continue
        added, deleted = diff_line_count(path, "7.days")
        if added + deleted > 400:
            if paired_adr_updated(path, changed):
                continue
            report.add(
                Finding(
                    severity="warn",
                    category="scope",
                    file=path,
                    rule="daily-audit — large change in safety-critical path (7d window)",
                    message=f"Large diff ({added}+{deleted} lines) in risky area over 7 days",
                )
            )


def check_adr_staleness(changed: list[str], report: Report) -> None:
    """Flag source edits whose mapped ADR/decision doc was not also changed.

    Credits the exact path on each ``AdrRule`` (including ``docs/safety.md`` for Davout).
    Source suffixes are per-rule (``.rs`` for crates, ``.proto`` for ``proto/``).
    Path membership only — not a substantive docs-quality check.
    """
    changed_set = set(changed)
    for rule in ADR_RULES:
        touched = _sources_for_rule(rule, changed)
        if touched and rule.adr not in changed_set:
            report.add(
                Finding(
                    severity="warn",
                    category="docs",
                    file=touched[0],
                    rule=f"ADR staleness — {rule.adr} may need update",
                    message=f"Changed under {rule.prefix} without ADR/decision doc update",
                )
            )


def check_hardware_config_coupling(changed: list[str], report: Report) -> None:
    motor_cfg = [p for p in changed if p.startswith("config/") and "motor" in p.lower()]
    kin = [p for p in changed if "kinematics" in p.lower()]
    docs = [p for p in changed if p.startswith("docs/") or p.startswith("hardware/docs/")]
    if (motor_cfg or kin) and not docs:
        report.add(
            Finding(
                severity="warn",
                category="config",
                file=(motor_cfg or kin)[0],
                rule="daily-audit-rubric R10 — config/kinematics needs doc pairing",
                message="Hardware/config changed without doc updates in scan window",
            )
        )


def check_ci_status(report: Report) -> None:
    repo = os.environ.get("GITHUB_REPOSITORY", "jaylamping/marengo")
    runs = run_json(
        [
            "gh",
            "run",
            "list",
            "--repo",
            repo,
            "--branch",
            "main",
            "--workflow",
            "CI",
            "--limit",
            "1",
            "--json",
            "databaseId,conclusion,headSha,url",
        ]
    )
    if not isinstance(runs, list) or not runs:
        return
    latest = runs[0]
    conclusion = latest.get("conclusion")
    if conclusion and conclusion != "success":
        report.add(
            Finding(
                severity="warn",
                category="ci",
                file="main",
                rule="AGENTS.md — keep CI green before shipping",
                message=f"Latest CI run on main: {conclusion} ({latest.get('url', '')})",
            )
        )


def check_stale_safety_prs(report: Report) -> None:
    repo = os.environ.get("GITHUB_REPOSITORY", "jaylamping/marengo")
    prs = run_json(
        [
            "gh",
            "pr",
            "list",
            "--repo",
            repo,
            "--state",
            "open",
            "--json",
            "number,createdAt,files,url",
            "--limit",
            "30",
        ]
    )
    if not isinstance(prs, list):
        return
    cutoff = datetime.now(timezone.utc) - timedelta(days=7)
    for pr in prs:
        created_raw = pr.get("createdAt")
        if not created_raw:
            continue
        created = datetime.fromisoformat(created_raw.replace("Z", "+00:00"))
        if created > cutoff:
            continue
        files = [f.get("path", "") for f in pr.get("files") or []]
        if any(any(path.startswith(p) for p in SAFETY_PREFIXES) for path in files):
            report.add(
                Finding(
                    severity="warn",
                    category="process",
                    file=f"PR #{pr.get('number')}",
                    rule="daily-audit-rubric — stale open PR on safety paths",
                    message=f"Open >7d with safety-path edits ({pr.get('url', '')})",
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


def write_report(report: Report, out_dir: Path) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    payload = {
        "date": report.date,
        "commits_reviewed": report.commits_reviewed,
        "changed_files": report.changed_files,
        "findings": [asdict(f) for f in report.findings],
        "topics": report.topics,
        "clean": report.clean,
        "scan_windows": report.scan_windows,
        "generated_at": datetime.now(timezone.utc).isoformat(),
    }
    (out_dir / "report.json").write_text(json.dumps(payload, indent=2), encoding="utf-8")
    lines = [
        f"# Daily audit {report.date}",
        "",
        f"**Clean:** {report.clean}",
        f"**Commits:** {len(report.commits_reviewed)}",
        f"**Changed files:** {len(report.changed_files)}",
        f"**Scan windows:** {report.scan_windows}",
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
    (out_dir / "report.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    (out_dir / "topics.json").write_text(json.dumps({"topics": report.topics}, indent=2), encoding="utf-8")
    print(
        json.dumps(
            {
                "out_dir": str(out_dir),
                "clean": report.clean,
                "findings": len(report.findings),
            }
        )
    )


def main() -> int:
    date_str = utc_today()
    out_dir = out_dir_for(date_str)
    commits, changed, windows = git_changed_files()
    report = Report(
        date=date_str,
        commits_reviewed=commits,
        changed_files=changed,
        scan_windows=windows,
    )
    if changed:
        check_unwrap(changed, report)
        check_gen_handedit(changed, report)
        check_proto_checksum(changed, report)
        check_davout_bypass(changed, report)
        check_large_risky_diff(changed, report)
        check_adr_staleness(changed, report)
        check_hardware_config_coupling(changed, report)
        report.topics = infer_topics(changed)
    check_ci_status(report)
    check_stale_safety_prs(report)
    write_report(report, out_dir)
    return 0 if report.clean else 1


if __name__ == "__main__":
    sys.exit(main())
