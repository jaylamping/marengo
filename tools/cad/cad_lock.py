#!/usr/bin/env python3
"""One CAD writer at a time in the shared checkout (J:\\code\\marengo on Joey-PC).

Many sessions share one working tree. Before a session opens SolidWorks to change files in
cad/, or stages cad/ files, it takes the writer lock; it releases it after its commit.
The lock is the gitignored file cad/.cad-writer.lock. The pre-commit hook refuses a commit
touching cad/ while someone else holds a live lock.

Usage:
    python tools/cad/cad_lock.py acquire OWNER [--note TEXT]   take the lock (or a stale one)
    python tools/cad/cad_lock.py refresh OWNER                 keep it alive during long work
    python tools/cad/cad_lock.py release OWNER                 give it back
    python tools/cad/cad_lock.py status                        who holds it
    python tools/cad/cad_lock.py check [OWNER]                 exit 0 if OWNER (default $CAD_WRITER)
                                                               may write: no live lock, or it is theirs
OWNER is the thread or person, e.g. "left-arm" or "joey". A lock untouched for 4 hours is
stale and the next acquire takes it over (the old owner is printed).
Exit code 0 = ok, 1 = held by someone else / not yours, 2 = bad usage.
"""

from __future__ import annotations

import json
import os
import socket
import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
LOCK_PATH = REPO_ROOT / "cad" / ".cad-writer.lock"
STALE_SECONDS = 4 * 3600


def read(path: Path = LOCK_PATH) -> dict | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return None
    except (OSError, ValueError):
        return {"owner": "(unreadable lock file)", "updated": 0}


def is_stale(lock: dict, now: float | None = None) -> bool:
    return (now or time.time()) - float(lock.get("updated", 0)) > STALE_SECONDS


def describe(lock: dict) -> str:
    age = (time.time() - float(lock.get("updated", 0))) / 60
    note = f", {lock['note']}" if lock.get("note") else ""
    return f"{lock.get('owner')} on {lock.get('host', '?')}, updated {age:.0f} min ago{note}"


def _write_new(path: Path, data: dict) -> bool:
    """Create the lock file atomically; False if it already exists."""
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
    except FileExistsError:
        return False
    with os.fdopen(fd, "w", encoding="utf-8") as fh:
        json.dump(data, fh)
    return True


def acquire(owner: str, note: str = "", path: Path = LOCK_PATH) -> tuple[bool, str]:
    now = time.time()
    data = {
        "owner": owner,
        "host": socket.gethostname(),
        "started": now,
        "updated": now,
        "note": note,
    }
    if _write_new(path, data):
        return True, f"lock taken by {owner}"
    lock = read(path) or {}
    if lock.get("owner") == owner:
        lock["updated"] = now
        path.write_text(json.dumps(lock), encoding="utf-8")
        return True, f"{owner} already holds the lock (refreshed)"
    if not is_stale(lock, now):
        return False, f"CAD writer lock held by {describe(lock)}"
    path.unlink(missing_ok=True)
    if _write_new(path, data):
        return True, f"took over a stale lock from {describe(lock)}"
    return False, "another session took the lock first"


def refresh(owner: str, path: Path = LOCK_PATH) -> tuple[bool, str]:
    lock = read(path)
    if not lock or lock.get("owner") != owner:
        return False, f"{owner} does not hold the lock"
    lock["updated"] = time.time()
    path.write_text(json.dumps(lock), encoding="utf-8")
    return True, "refreshed"


def release(owner: str, path: Path = LOCK_PATH) -> tuple[bool, str]:
    lock = read(path)
    if not lock:
        return True, "no lock held"
    if lock.get("owner") != owner:
        return False, f"lock belongs to {describe(lock)}, not {owner}"
    path.unlink(missing_ok=True)
    return True, f"released by {owner}"


def check(owner: str | None, path: Path = LOCK_PATH) -> tuple[bool, str]:
    lock = read(path)
    if not lock or is_stale(lock):
        return True, "no live CAD writer lock"
    if owner and lock.get("owner") == owner:
        return True, f"{owner} holds the lock"
    return False, (
        f"CAD writer lock held by {describe(lock)}. Wait for it, or ask that session to release it "
        "(python tools/cad/cad_lock.py release OWNER). Set CAD_WRITER=<owner> if it is yours."
    )


def main(argv: list[str]) -> int:
    if not argv:
        print(__doc__)
        return 2
    cmd, rest = argv[0], argv[1:]
    note = ""
    if "--note" in rest:
        i = rest.index("--note")
        note = " ".join(rest[i + 1 :])
        rest = rest[:i]
    if cmd == "status" and not rest:
        lock = read()
        print(
            f"held by {describe(lock)}{' (stale)' if is_stale(lock) else ''}"
            if lock
            else "no lock held"
        )
        return 0
    if cmd == "check" and len(rest) <= 1:
        ok, msg = check(rest[0] if rest else os.environ.get("CAD_WRITER"))
    elif cmd in {"acquire", "refresh", "release"} and len(rest) == 1:
        ok, msg = {
            "acquire": lambda o: acquire(o, note),
            "refresh": refresh,
            "release": release,
        }[cmd](rest[0])
    else:
        print(__doc__)
        return 2
    print(msg)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
