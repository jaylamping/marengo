#!/usr/bin/env bash
set -euo pipefail
validation=/home/joey/marengo-validation
stack="$validation/batch35-20261002/stack-8e1871f"
python3 - <<'PY'
import hashlib,json,tarfile
from pathlib import Path
validation=Path('/home/joey/marengo-validation')
stack=validation/'batch35-20261002/stack-8e1871f'
manifest=json.loads((validation/'batch35-pi-source-8e1871f-manifest.json').read_text())
assert manifest['source']=='8e1871f64ebcb7445bd83a241e6b2d67705202f1'
assert stack.resolve()==stack
archive=validation/'batch35-pi-source-8e1871f.tar.gz'
assert hashlib.sha256(archive.read_bytes()).hexdigest()==manifest['archive_sha256']
stack.mkdir(parents=True,exist_ok=True)
with tarfile.open(archive,'r:gz') as tar:
    members=[]
    for item in tar.getmembers():
        assert item.isdir() or item.isfile(),item.name
        assert not Path(item.name).is_absolute() and '..' not in Path(item.name).parts,item.name
        target=stack/item.name
        assert target.resolve()==target,item.name
        if item.isfile() and target.is_file():
            assert hashlib.sha256(target.read_bytes()).hexdigest()==manifest['file_sha256'][item.name],item.name
            continue
        members.append(item)
    tar.extractall(stack,members=members,filter='data')
for name,expected in manifest['file_sha256'].items():
    assert hashlib.sha256((stack/name).read_bytes()).hexdigest()==expected,name
print('Native source verified:',len(manifest['file_sha256']),'files at',manifest['source'],flush=True)
PY
cd "$stack"
export PATH=/home/joey/.cargo/bin:/home/joey/marengo-validation/batch27-20261001/tools/node-v24.16.0-linux-arm64/bin:$PATH
export RUSTUP_TOOLCHAIN=1.88.0 CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=/home/joey/marengo-validation/batch27-20261001/stack-3cde431/target
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=cc
export MARENGO_ROOT="$stack" MARENGO_CONFIG_DIR="$stack/config"
export GITHUB_SHA=8e1871f64ebcb7445bd83a241e6b2d67705202f1
nice -n 10 cargo test --locked --workspace
cd consul
export NODE_OPTIONS=--max-old-space-size=1024
nice -n 10 npm ci
npm run gen:proto
../scripts/proto-checksum.sh
nice -n 10 npm test -- --run --maxWorkers=1
nice -n 10 npm run build:qualified -- --ignore-scripts
../scripts/check-consul-dist.sh
cd "$stack"
python3 - <<'PY'
import hashlib,json
from pathlib import Path
manifest=json.loads(Path('/home/joey/marengo-validation/batch35-pi-source-8e1871f-manifest.json').read_text())
for name,expected in manifest['file_sha256'].items():
    assert hashlib.sha256(Path(name).read_bytes()).hexdigest()==expected,name
print('Post-test committed inputs unchanged:',len(manifest['file_sha256']),flush=True)
PY
cat /opt/marengo/.deploy-rev
systemctl is-active marengo-pi.service marengo-gateway.service marengo-can.service
free -m
