#!/usr/bin/env python3
"""Install the repo's git hooks (pre-commit, pre-push) into this clone.

    python tools/cad/install_hooks.py            install or update
    python tools/cad/install_hooks.py --check    exit 1 if they are missing or out of date

pre-commit: tools/cad/hooks/pre_commit.py (staged sizes; layout + writer lock when CAD is staged).
pre-push:   tools/cad/hooks/pre_push.py (pushed sizes, LFS budget), then `git lfs pre-push`
            (fails if Git LFS is missing), then <hook>.local if present.
Does not set core.hooksPath. Git LFS's own hook is replaced (ours runs it; a copy is kept as
<hook>.lfs-orig); any other existing hook is kept as <hook>.local and still runs after ours.
Run it once per clone (Joey-PC, Mac); re-run after `git lfs install`.
"""

from __future__ import annotations

import os
import stat
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gitlfs import REPO_ROOT, git

MARK = "# marengo-cad-hooks v2"

FIND_PY = """for c in python3 python py; do
  if command -v "$c" >/dev/null 2>&1 && "$c" -c "import sys; sys.exit(sys.version_info < (3, 9))" >/dev/null 2>&1; then
    PY=$c; break
  fi
done
if [ -z "$PY" ]; then
  echo "marengo hooks: Python 3.9+ not found; install it (or bypass once with --no-verify)" >&2
  exit 1
fi
TOP=$(git rev-parse --show-toplevel)
HOOKS=$(git rev-parse --git-path hooks)
"""

HOOKS = {
    "pre-commit": f"""#!/bin/sh
{MARK}
{FIND_PY}"$PY" "$TOP/tools/cad/hooks/pre_commit.py" || exit 1
if [ -x "$HOOKS/pre-commit.local" ]; then
  exec "$HOOKS/pre-commit.local" "$@"
fi
exit 0
""",
    "pre-push": f"""#!/bin/sh
{MARK}
{FIND_PY}input=$(cat)
printf '%s\\n' "$input" | "$PY" "$TOP/tools/cad/hooks/pre_push.py" "$@" || exit 1
# Git LFS must upload the objects, or the remote gets pointers with no content.
if ! command -v git-lfs >/dev/null 2>&1; then
  echo "marengo hooks: git-lfs not found; install Git LFS before pushing (https://git-lfs.com)" >&2
  exit 1
fi
printf '%s\\n' "$input" | git lfs pre-push "$@" || exit 1
if [ -x "$HOOKS/pre-push.local" ]; then
  printf '%s\\n' "$input" | "$HOOKS/pre-push.local" "$@" || exit 1
fi
exit 0
""",
}


def hooks_dir(root: Path = REPO_ROOT) -> Path:
    p = Path(git("rev-parse", "--git-path", "hooks", root=root).decode().strip())
    return p if p.is_absolute() else root / p


def install(root: Path = REPO_ROOT) -> list[str]:
    d = hooks_dir(root)
    d.mkdir(parents=True, exist_ok=True)
    done = []
    for name, body in HOOKS.items():
        path = d / name
        if path.exists():
            old = path.read_text(encoding="utf-8", errors="replace")
            if "# marengo-cad-hooks" in old:
                pass  # an older version of ours: overwrite
            elif "git lfs" in old and len(old) < 1024:
                # Git LFS's own hook: ours already runs `git lfs pre-push`; keep a copy that does not run
                path.replace(d / f"{name}.lfs-orig")
                done.append(
                    f"replaced Git LFS's {name} hook (ours runs git lfs; copy in {name}.lfs-orig)"
                )
            else:
                local = d / f"{name}.local"
                if local.exists():
                    raise SystemExit(
                        f"{path} is not ours and {local} already exists; merge them by hand"
                    )
                path.rename(local)
                done.append(f"kept the existing {name} hook as {local.name}")
        with path.open("w", encoding="utf-8", newline="\n") as fh:
            fh.write(body)
        path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        done.append(f"installed {name}")
    return done


def is_current(root: Path = REPO_ROOT) -> bool:
    d = hooks_dir(root)
    for name, body in HOOKS.items():
        path = d / name
        if (
            not path.exists()
            or path.read_text(encoding="utf-8", errors="replace") != body
        ):
            return False
        if os.name != "nt" and not os.access(path, os.X_OK):
            return False
    return True


def main(argv: list[str]) -> int:
    if argv == ["--check"]:
        ok = is_current()
        print(
            "hooks installed and current"
            if ok
            else "hooks missing or out of date: python tools/cad/install_hooks.py"
        )
        return 0 if ok else 1
    if argv:
        print(__doc__)
        return 2
    for line in install():
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
