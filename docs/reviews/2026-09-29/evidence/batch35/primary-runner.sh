#!/usr/bin/env bash
set -euo pipefail
python3 - <<'MODE'
import os,subprocess
for entry in subprocess.check_output(['git','-c','safe.directory=/workspace','ls-files','--stage','-z']).split(b'\0'):
 if entry.startswith(b'100755 '):
  name=os.fsdecode(entry.split(b'\t',1)[1]); os.chmod(name,0o755)
MODE
export CI=true GITHUB_REF=refs/heads/main GITHUB_SHA=8e1871f64ebcb7445bd83a241e6b2d67705202f1
bash ./scripts/check.sh
