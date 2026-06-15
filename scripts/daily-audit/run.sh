#!/usr/bin/env sh
# Daily deterministic audit entrypoint.
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
DATE="$(date -u +%Y-%m-%d)"
OUT_DIR="$ROOT/var/log/daily-audit/$DATE"
mkdir -p "$OUT_DIR"

echo "Running Marengo daily audit..."
python3 "$ROOT/scripts/daily-audit/audit.py" || true

# Optional cargo audit (non-fatal if unavailable)
if command -v cargo-audit >/dev/null 2>&1; then
  if cargo audit --disable-fetch 2>"$OUT_DIR/cargo-audit.log"; then
    echo "cargo audit: ok"
  else
    echo "cargo audit: findings (see $OUT_DIR/cargo-audit.log)" >> "$OUT_DIR/report.md"
    python3 - <<'PY'
import json
from pathlib import Path
import os
root = Path(os.environ.get("ROOT", "."))
out = root / "var/log/daily-audit" / __import__("datetime").date.today().isoformat()
report = json.loads((out / "report.json").read_text())
report["findings"].append({
    "severity": "warn",
    "category": "deps",
    "file": "Cargo.lock",
    "rule": "cargo audit — dependency advisories",
    "message": "cargo audit reported advisories; see cargo-audit.log",
    "commit": "",
})
report["clean"] = False
(out / "report.json").write_text(json.dumps(report, indent=2))
PY
  fi
fi

# Optional research appendix when marengo-research-mcp is set up
if [ -f "$OUT_DIR/topics.json" ] && command -v uv >/dev/null 2>&1; then
  if [ -d "$ROOT/tools/marengo-research-mcp" ]; then
    (cd "$ROOT/tools/marengo-research-mcp" && uv run python -m marengo_research_mcp.cli audit-research \
      --topics-file "$OUT_DIR/topics.json" \
      -o "$OUT_DIR/research.md") || echo "research appendix skipped"
  fi
fi

echo "Report: $OUT_DIR/report.json"
