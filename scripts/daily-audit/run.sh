#!/usr/bin/env sh
# Daily deterministic audit entrypoint.
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
DATE="$(date -u +%Y-%m-%d)"
OUT_DIR="$ROOT/var/log/daily-audit/$DATE"
mkdir -p "$OUT_DIR"

echo "Running Marengo daily audit..."
PYTHON=""
for cmd in python3 python py; do
  if command -v "$cmd" >/dev/null 2>&1; then
    PYTHON="$cmd"
    break
  fi
done
if [ -z "$PYTHON" ]; then
  echo "No python interpreter found (tried python3, python, py)" >&2
  exit 1
fi
"$PYTHON" "$ROOT/scripts/daily-audit/audit.py" || true

# Optional offline advisory scan. An incomplete scan is never a clean result.
"$PYTHON" - "$OUT_DIR" <<'PY'
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from datetime import datetime, timezone

out = Path(sys.argv[1])
db = Path(os.environ.get("MARENGO_ADVISORY_DB", str(Path.home() / ".cargo/advisory-db")))
command = ["cargo", "audit", "--no-fetch", "--no-yanked", "--json", "--db", str(db)]
result = {"status": "unavailable", "command": command, "exit_code": None,
          "database_path": str(db), "snapshot_status": "unknown",
          "coverage": "RustSec advisories and maintenance warnings; yanked registry scan excluded",
          "checked_at": datetime.now(timezone.utc).isoformat()}
stdout = stderr = ""
if shutil.which("cargo-audit") and shutil.which("cargo"):
    try:
        proc = subprocess.run(command, capture_output=True, text=True, timeout=120)
        stdout, stderr = proc.stdout, proc.stderr
        result["exit_code"] = proc.returncode
        result["status"] = "error"
        if proc.returncode in (0, 1):
            payload = json.loads(stdout)
            vulnerabilities = payload["vulnerabilities"]
            count, items, found = vulnerabilities["count"], vulnerabilities["list"], vulnerabilities["found"]
            warnings = payload["warnings"]
            if (type(count) is not int or count < 0 or not isinstance(items, list)
                    or len(items) != count or type(found) is not bool or found != bool(count)
                    or not isinstance(warnings, dict) or any(not isinstance(v, list) for v in warnings.values())):
                raise ValueError("invalid cargo-audit report shape")
            result["vulnerability_count"] = count
            result["maintenance_warning_count"] = sum(len(v) for v in warnings.values())
            database = payload["database"]
            if database.get("last-updated") is None or database.get("last-commit") is None:
                # cargo-audit --no-fetch may omit provenance; require a clean owned Git snapshot.
                status = subprocess.run(["git", "-C", str(db), "status", "--porcelain"], capture_output=True, text=True, timeout=10)
                metadata = subprocess.run(["git", "-C", str(db), "log", "-1", "--format=%H%n%cI"], capture_output=True, text=True, timeout=10)
                if status.returncode != 0 or status.stdout.strip() or metadata.returncode != 0:
                    raise ValueError("database provenance unavailable or snapshot modified")
                commit, updated = metadata.stdout.strip().splitlines()
                database = {"last-commit": commit, "last-updated": updated}
                result["provenance_source"] = "clean prepared Git snapshot"
            else:
                result["provenance_source"] = "cargo-audit report"
            stamp = datetime.fromisoformat(database["last-updated"].replace("Z", "+00:00"))
            if stamp.tzinfo is None or not database["last-commit"]:
                raise ValueError("database provenance missing")
            age = (datetime.now(timezone.utc) - stamp).total_seconds()
            result.update(database_commit=database["last-commit"], database_updated_at=stamp.isoformat(), database_age_seconds=age)
            # Offline snapshot policy: last committed <=7 days ago, never future.
            result["snapshot_status"] = "current" if 0 <= age <= 7 * 86400 else "stale"
            if result["snapshot_status"] == "stale":
                result["status"] = "stale-database"
            elif count:
                result["status"] = "vulnerabilities" if proc.returncode == 1 else "error"
            elif result["maintenance_warning_count"]:
                result["status"] = "maintenance-warnings"
            elif proc.returncode == 0:
                result["status"] = "clean"
    except (OSError, ValueError, KeyError, TypeError, AttributeError, subprocess.TimeoutExpired) as exc:
        result["status"] = "error"
        result["error"] = str(exc)
    if not db.is_dir():
        result["snapshot_status"] = "missing"
        if result["status"] != "error":
            result["status"] = "missing-database"
else:
    result["error"] = "cargo or cargo-audit unavailable"
(out / "cargo-audit.stdout.json").write_text(stdout, encoding="utf-8")
(out / "cargo-audit.stderr.log").write_text(stderr, encoding="utf-8")
(out / "cargo-audit-result.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
report_path = out / "report.json"
if report_path.exists():
    report = json.loads(report_path.read_text())
    report["cargo_audit"] = result
    if result["status"] != "clean":
        report["clean"] = False
        report["findings"].append({"severity": "warn", "category": "deps", "file": "Cargo.lock",
            "rule": "cargo audit — classified offline scan", "commit": "",
            "message": f"cargo audit status: {result['status']}; see cargo-audit-result.json"})
    report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
with (out / "report.md").open("a", encoding="utf-8") as report_md:
    report_md.write(f"\nCargo audit: {result['status']} (snapshot: {result['snapshot_status']}); see cargo-audit-result.json.\n")
PY

# Optional research appendix when marengo-research-mcp is set up
if [ -f "$OUT_DIR/topics.json" ] && command -v uv >/dev/null 2>&1; then
  if [ -d "$ROOT/tools/marengo-research-mcp" ]; then
    (cd "$ROOT/tools/marengo-research-mcp" && uv run python -m marengo_research_mcp.cli audit-research \
      --topics-file "$OUT_DIR/topics.json" \
      -o "$OUT_DIR/research.md") || echo "research appendix skipped"
  fi
fi

echo "Report: $OUT_DIR/report.json"
