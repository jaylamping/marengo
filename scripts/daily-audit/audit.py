#!/usr/bin/env python3
"""Daily audit report builder."""

from __future__ import annotations

import json
import hashlib
import os
import re
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from datetime import datetime, timedelta, timezone
from pathlib import Path

from rust_scan import ScanUnknown, production_view
from generated_scan import verify_generated
from defect_ledger import record_findings

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
    checks: dict[str, dict] = field(default_factory=dict)
    unresolved_findings: list[dict] = field(default_factory=list)
    evidence: list[dict] = field(default_factory=list)

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
    if result.returncode != 0:
        raise RuntimeError(f"command failed ({result.returncode}): {cmd[0]}")
    return result.stdout.strip()


def run_json(cmd: list[str], cwd: Path = ROOT) -> list | dict | None:
    proc = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, check=False)
    if proc.returncode != 0 or not proc.stdout.strip():
        raise RuntimeError(f"JSON command failed/incomplete ({proc.returncode}): {cmd[0]}")
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
    """Return a lexical production view; unsupported cfg is explicitly unknown."""
    return production_view(source)


def production_rust_lines(source: str) -> str:
    return production_view(source)


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
    if not any(path.startswith(("proto/", "consul/src/gen/")) for path in changed):
        return
    try:
        proof = verify_generated(ROOT, changed)
    except (OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as exc:
        proof = {"status": "unknown", "detail": str(exc)}
    report.evidence.append({"check": "generated_code", **proof})
    if proof["status"] == "verified":
        return
    mismatch = proof["status"] == "mismatch"
    report.add(Finding(severity="critical" if mismatch else "warn", category="proto" if mismatch else "scan",
        file="consul/src/gen/", rule="AGENTS.md R3 — generated code must match locked regeneration",
        message=f"{proof['status']}: {proof['detail']}; mismatches={proof.get('mismatches', [])}"))


def check_davout_bypass(changed: list[str], report: Report) -> None:
    if "crates/berthier/Cargo.toml" in changed:
        proc = subprocess.run(["cargo", "metadata", "--offline", "--locked", "--no-deps", "--format-version", "1"],
            cwd=ROOT, capture_output=True, text=True, timeout=30)
        if proc.returncode != 0:
            raise RuntimeError("Cargo dependency evidence unavailable")
        metadata = json.loads(proc.stdout)
        packages = metadata["packages"]
        package = next((p for p in packages if p["name"] == "berthier"), None)
        if package is None:
            raise ValueError("Berthier missing from dependency inventory")
        report.evidence.append({"check": "berthier_dependencies", "metadata_sha256": hashlib.sha256(proc.stdout.encode()).hexdigest()})
        for dependency in package["dependencies"]:
            if dependency["name"] in {"robstride", "socketcan"} and dependency["kind"] != "dev":
                report.add(Finding("critical", "safety", "crates/berthier/Cargo.toml",
                    "docs/architecture.md R6 — production driver dependency",
                    f"Production dependency {dependency['name']} (alias={dependency.get('rename')})"))
    for path in changed:
        if not path.startswith("crates/berthier/") or not path.endswith(".rs") or "/tests/" in path:
            continue
        full = ROOT / path
        if not full.is_file():
            continue
        try:
            source = full.read_text(encoding="utf-8")
            body = production_view(source)
            report.evidence.append({"check": "production_rust", "file": path,
                "source_sha256": hashlib.sha256(source.encode()).hexdigest(), "line_numbers_preserved": True})
        except (OSError, UnicodeError, ScanUnknown) as exc:
            report.add(Finding(severity="warn", category="scan", file=path,
                rule="production Rust classification", message=f"Unknown: {exc}"))
            continue
        for match in re.finditer(r"\b(?:robstride|socketcan)\s*::", body):
            line = body.count("\n", 0, match.start()) + 1
            report.add(Finding(severity="critical", category="safety", file=path,
                rule="docs/architecture.md R6 — Berthier must not touch CAN/robstride",
                message=f"Production driver path at line {line}: {match.group().strip()}"))


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
        raise ValueError("CI evidence unavailable")
    latest = runs[0]
    conclusion = latest.get("conclusion")
    if not conclusion:
        raise ValueError("CI run has no completed conclusion")
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
        raise ValueError("PR inventory evidence unavailable")
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
        "unresolved_findings": report.unresolved_findings,
        "evidence": report.evidence,
        "topics": report.topics,
        "clean": report.clean,
        "scan_windows": report.scan_windows,
        "checks": report.checks,
        "completeness": "complete" if report.checks and all(c["status"] == "complete" for c in report.checks.values()) else "unknown",
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
        f"**Check completeness:** {payload['completeness']}",
        f"**Unresolved durable findings:** {len(report.unresolved_findings)}",
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


def capture_check(report: Report, name: str, operation) -> None:
    before = len(report.findings)
    try:
        operation()
        uncertain = any(f.category == "scan" for f in report.findings[before:])
        report.checks[name] = {"status": "unknown" if uncertain else "complete"}
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
        report.checks[name] = {"status": "failed", "error": str(exc)}
        report.add(Finding(severity="warn", category="scan", file=name,
            rule="daily audit completeness", message=f"Failed: {exc}"))


def main() -> int:
    report = Report(date=utc_today())
    def inventory():
        commits, changed, windows = git_changed_files()
        report.commits_reviewed, report.changed_files, report.scan_windows = commits, changed, windows
    capture_check(report, "git_inventory", inventory)
    for checker in (check_unwrap, check_gen_handedit, check_proto_checksum, check_davout_bypass,
                    check_large_risky_diff, check_adr_staleness, check_hardware_config_coupling):
        if report.checks["git_inventory"]["status"] == "complete":
            capture_check(report, checker.__name__, lambda checker=checker: checker(report.changed_files, report))
        else:
            report.checks[checker.__name__] = {"status": "unknown", "reason": "inventory failed"}
    report.topics = infer_topics(report.changed_files)
    capture_check(report, "ci_status", lambda: check_ci_status(report))
    capture_check(report, "stale_safety_prs", lambda: check_stale_safety_prs(report))
    def persist_findings():
        report.unresolved_findings = record_findings(ROOT / "var/log/daily-audit/defects.json",
            [asdict(f) for f in report.findings], datetime.now(timezone.utc).isoformat())
        if report.unresolved_findings:
            report.clean = False
    capture_check(report, "durable_findings", persist_findings)
    write_report(report, out_dir_for(report.date))
    return 0 if report.clean and all(c["status"] == "complete" for c in report.checks.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
