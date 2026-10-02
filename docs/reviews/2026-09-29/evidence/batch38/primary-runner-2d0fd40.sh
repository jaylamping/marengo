#!/usr/bin/env bash
set -euo pipefail
python3 - <<'MODE'
import os,subprocess
for entry in subprocess.check_output(['git','-c','safe.directory=/workspace','ls-files','--stage','-z']).split(b'\0'):
 if entry.startswith(b'100755 '):
  name=os.fsdecode(entry.split(b'\t',1)[1]); os.chmod(name,0o755)
MODE
export CI=true GITHUB_REF=refs/heads/main GITHUB_SHA=2d0fd4088b0ca0301b2e27b07560b54b40ec61ca
/usr/local/bin/docker-entrypoint.sh bash ./scripts/check.sh

python3 - <<'BOUND'
import hashlib,json
from pathlib import Path
manifest=json.loads(Path('/evidence/pi-source-2d0fd40-manifest.json').read_text())
assert manifest['source']=='2d0fd4088b0ca0301b2e27b07560b54b40ec61ca'
for name,expected in manifest['file_sha256'].items():
 assert hashlib.sha256(Path(name).read_bytes()).hexdigest()==expected,name
print('Post-test committed inputs unchanged:',len(manifest['file_sha256']),flush=True)
BOUND
