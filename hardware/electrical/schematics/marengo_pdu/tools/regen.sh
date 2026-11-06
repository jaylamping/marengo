#!/bin/bash
# Regenerate the sheets, then run ERC, the netlist check and the wire scan. Usage: tools/regen.sh [output dir]
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:-$ROOT}
K=/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli
TMP=$(mktemp -d)
python3 -I "$ROOT/gen/gen.py" "$OUT"
[ "$OUT" = "$ROOT" ] || cp -R "$ROOT/marengo.pretty" "$ROOT/fp-lib-table" "$OUT/"
cd "$OUT"
"$K" sch erc --severity-all --format report -o "$TMP/erc.rpt" marengo_pdu.kicad_sch 2>/dev/null | tail -1
grep 'ERC messages' "$TMP/erc.rpt"
"$K" sch export netlist --format kicadsexpr -o "$TMP/net.net" marengo_pdu.kicad_sch 2>/dev/null | tail -1
python3 -I "$ROOT/tools/check.py" "$ROOT/tools" "$TMP/net.net" "$ROOT/marengo.pretty" | grep -E '^FAIL|^audited|^FAILS'
python3 -I "$ROOT/tools/wirescan.py" "$OUT" | grep -v PWR_FLAG || true
echo "netlist: $TMP/net.net"
