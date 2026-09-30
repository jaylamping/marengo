#!/usr/bin/env bash
# Update loaded-side mass in the archived weighted URDF after scale measurement.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: bench-set-weighted-mass.sh <mass_kg> <right|left>

Example (right side dowel + 500 g plate ≈ 0.583 kg total):
  ./scripts/bench-set-weighted-mass.sh 0.583 right

Then sync to Pi: pi_sync_bench_config profile=shoulder_pitch_weighted
EOF
}

if [[ $# -ne 2 ]]; then
  usage
  exit 1
fi

mass="$1"
side="$2"
repo_root="$(cd "$(dirname "$0")/.." && pwd)"
urdf="${repo_root}/assets/urdf/archive/seed-shoulder_pitch_weighted/contributor.urdf"

if ! [[ "$mass" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
  echo "invalid mass_kg: $mass" >&2
  exit 1
fi

case "$side" in
  right) link="right_upper_arm_stub" ;;
  left) link="left_upper_arm_stub" ;;
  *)
    echo "side must be right or left" >&2
    exit 1
    ;;
esac

python3 - "$urdf" "$link" "$mass" <<'PY'
import re
import sys

path, link, mass = sys.argv[1:4]
text = open(path, encoding="utf-8").read()
pattern = (
    rf'(<link name="{re.escape(link)}">\s*<inertial>\s*'
    rf'(?:<!--.*?-->\s*)?'
    rf'<mass value=")[^"]+(")'
)
new_text, n = re.subn(pattern, rf"\g<1>{mass}\2", text, count=1, flags=re.DOTALL)
if n != 1:
    sys.exit(f"could not update mass for {link} in {path}")
open(path, "w", encoding="utf-8").write(new_text)
print(f"updated {link} mass → {mass} kg in {path}")
PY
