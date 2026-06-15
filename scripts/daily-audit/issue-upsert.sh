#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DATE="${DATE:-$(date -u +%Y-%m-%d)}"
REPORT="$ROOT/var/log/daily-audit/$DATE/report.json"
if [ ! -f "$REPORT" ]; then
  echo "No report at $REPORT"
  exit 0
fi

CLEAN="$(python3 -c "import json; print(json.load(open('$REPORT'))['clean'])")"
FINDINGS="$(python3 -c "import json; d=json.load(open('$REPORT')); print(len([f for f in d['findings'] if f['severity'] in ('warn','critical')]))")"
BODY_FILE="$(mktemp)"
python3 - "$REPORT" "$BODY_FILE" <<'PY'
import json, sys
report = json.load(open(sys.argv[1]))
lines = [f"# Daily audit {report['date']}", "", f"Findings (warn+): {len([f for f in report['findings'] if f['severity'] in ('warn','critical')])}", ""]
for f in report["findings"]:
    if f["severity"] not in ("warn", "critical"):
        continue
    lines.append(f"- **{f['severity']}** `{f['file']}` — {f['message']} ({f['rule']})")
open(sys.argv[2], "w").write("\n".join(lines) + "\n")
PY

if [ "$CLEAN" = "True" ] || [ "$FINDINGS" = "0" ]; then
  ISSUE_NUM="$(gh issue list --label daily-audit --state open --json number --jq '.[0].number // empty')"
  if [ -n "$ISSUE_NUM" ]; then
    gh issue comment "$ISSUE_NUM" --body "All clear on $DATE (deterministic audit)."
    gh issue close "$ISSUE_NUM" --comment "Auto-closed: clean deterministic audit on $DATE."
  fi
  exit 0
fi

TITLE="Daily audit: $FINDINGS findings ($DATE)"
EXISTING="$(gh issue list --label daily-audit --state open --json number --jq '.[0].number // empty')"
if [ -n "$EXISTING" ]; then
  gh issue comment "$EXISTING" --body-file "$BODY_FILE"
else
  gh issue create --label daily-audit --title "$TITLE" --body-file "$BODY_FILE"
fi

python3 "$ROOT/scripts/daily-audit/pr-comment.py" "$REPORT" || true
rm -f "$BODY_FILE"
