import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(r"J:\code\marengo-worktrees\current-virtual-reference")
OUT = Path(r"J:\code\marengo-migration-backup-20260929\batch33-pi-install-log-links")
COMMIT = "0b2eb75e3922284da4ecdc4ad16561a67c777235"
source = subprocess.check_output(["git", "show", COMMIT + ":scripts/install-pi.sh"], cwd=ROOT)
probe = subprocess.check_output(["git", "show", COMMIT + ":scripts/test_install_permissions.py"], cwd=ROOT)
guard = b'  [[ "${target%/*}" == "${INSTALL_ROOT}/var/log" && -f "$target" ]]\n'
assert source.count(guard) == 1
mutant = source.replace(guard, b"  return 0\n", 1)
(OUT / "final-mutant-install-pi.sh").write_bytes(mutant)
receipt = {
    "source": COMMIT,
    "baseline_source": "d224cbd4b55faa5016f7c51ffb0447440950d550",
    "source_installer_sha256": hashlib.sha256(source).hexdigest(),
    "frozen_probe_sha256": hashlib.sha256(probe).hexdigest(),
    "baseline_installer_sha256": hashlib.sha256((OUT / "baseline-install-pi.sh").read_bytes()).hexdigest(),
    "mutation": "Omit same-directory/regular-target guard after actual readlink resolution",
    "mutant_installer_sha256": hashlib.sha256(mutant).hexdigest(),
    "positive_oracle": "InstallPermissions.test_installer_preserves_local_runtime_log_links",
    "negative_oracle": "InstallPermissions.test_installer_refuses_runtime_log_links_outside_local_regular_files",
    "source_and_probe_edited_for_mutation": False,
}
(OUT / "final-frozen-mutation-inputs.json").write_bytes((json.dumps(receipt, indent=2) + "\n").encode())
print(json.dumps(receipt, indent=2))
