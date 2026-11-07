#!/bin/bash
# Regenerate the sheet, run ERC, netlist check and wire scan. Usage: tools/regen.sh
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
K=/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli
TMP=$(mktemp -d)
cd "$ROOT"
python3 -I gen/genfp.py marengo_revb.pretty
python3 -I gen/gen.py "$ROOT"
"$K" sch erc --severity-all --format report -o "$TMP/erc.rpt" marengo_pdu_revb.kicad_sch 2>/dev/null | tail -1
grep -E 'ERC messages' "$TMP/erc.rpt"
grep -E '^\[' "$TMP/erc.rpt" | sort | uniq -c | sort -rn | head -20 || true
"$K" sch export netlist --format kicadsexpr -o "$TMP/net.net" marengo_pdu_revb.kicad_sch 2>/dev/null | tail -1
[ -f tools/check.py ] && python3 -I tools/check.py tools "$TMP/net.net" | grep -E '^FAIL|^FAILS|^audited'
python3 -I tools/wirescan.py "$ROOT" | grep -v PWR_FLAG || true
echo "netlist: $TMP/net.net"; echo "erc: $TMP/erc.rpt"
