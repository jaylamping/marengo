#!/bin/bash
# Rebuild the rev B5 board from the netlist: placement, power stage, spacing, GND fanout, signal routing, import, checks.
# Usage: tools/build_board.sh <work dir> [from-stage]   (stages: place power route import; default place; STOP=power ends
# after the base board)
# The work dir needs net.net (kicad-cli sch export netlist). Each stage's output is the next one's input:
#   place  -> placed.kicad_pcb          (tools/place.py)
#   power  -> base.kicad_pcb            (route_power, route_12v, hv_spacing, gnd_fanout) + copper.json
#   route  -> routes.json               (tools/grid_route.py, plain Python, about an hour)
#   import -> routed.kicad_pcb + drc.json + checks.txt (route_import, kicad-cli DRC with the project rules, route_checks)
set -uo pipefail
W=$(cd "$1" && (pwd -W 2>/dev/null || pwd)); FROM=${2:-place}   # pwd -W: a Windows path for the Windows Pythons
ROOT=$(cd "$(dirname "$0")/.." && pwd); LAYOUT="$ROOT/../../pdb/design/layout"
KP="/c/Program Files/KiCad/10.0/bin/python.exe"; KC="/c/Program Files/KiCad/10.0/bin/kicad-cli.exe"
[ -x "$KP" ] || { KP=/Applications/KiCad/KiCad.app/Contents/Frameworks/Python.framework/Versions/Current/bin/python3; KC=/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli; }
export MSYS_NO_PATHCONV=1
q() { grep -v 'memory leak'; }
cd "$ROOT"
run() { case "$FROM" in place) return 0;; power) [ "$1" != place ];; route) [ "$1" = route ] || [ "$1" = import ];; import) [ "$1" = import ];; esac; }
if run place; then
  "$KP" tools/place.py "$W/net.net" "$W/placed.kicad_pcb" "$W/place.json" 2>&1 | q | grep -E 'overlaps|missing|room|gap in'
fi
if run power; then
  "$KP" tools/route_power.py "$W/placed.kicad_pcb" "$W/base.kicad_pcb" 2>&1 | q | tail -1
  "$KP" tools/route_12v.py "$W/base.kicad_pcb" 2>&1 | q | tail -1
  "$KP" tools/hv_spacing.py "$W/base.kicad_pcb" > "$W/hv_spacing.txt" 2>&1 || echo "hv_spacing exit $?"; grep -E '^moved|^left|guard|removed' "$W/hv_spacing.txt" | cut -c1-160
  "$KP" tools/gnd_fanout.py "$W/base.kicad_pcb" 2>&1 | q | cut -c1-160 | tail -1
  "$KP" tools/route_export.py "$W/base.kicad_pcb" "$W/copper.json" 2>&1 | q | tail -1
  [ -s "$W/copper.json" ] || { echo 'power stage failed'; exit 1; }
  [ "${STOP:-}" = power ] && exit 0   # STOP=power: build the base board only
fi
if run route; then
  python -u tools/grid_route.py "$W/copper.json" "$W/routes.json" > "$W/route.log" 2>&1; grep -E '^done|Traceback' "$W/route.log" | cut -c1-400
  [ -s "$W/routes.json" ] || { echo 'routing failed'; exit 1; }
fi
if run import; then
  "$KP" tools/route_import.py "$W/base.kicad_pcb" "$W/routes.json" "$W/routed.kicad_pcb" 2>&1 | q | cut -c1-300
  cp "$LAYOUT/marengo_pdu_revb.kicad_pro" "$W/routed.kicad_pro"; cp "$LAYOUT/marengo_pdu_revb.kicad_dru" "$W/routed.kicad_dru"
  "$KC" pcb drc --format json --schematic-parity -o "$W/drc.json" "$W/routed.kicad_pcb" >/dev/null 2>&1
  python -c "import json,collections,sys;d=json.load(open(sys.argv[1]));print('DRC',dict(collections.Counter(v['type'] for v in d['violations'])),'unconnected',len(d['unconnected_items']))" "$W/drc.json"
  "$KP" tools/route_checks.py "$W/routed.kicad_pcb" 2>&1 | q > "$W/checks.txt"; grep -v '^    (' "$W/checks.txt"
fi
