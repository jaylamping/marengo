#!/usr/bin/env python3
"""One CAD writer at a time in the shared checkout (J:\\code\\marengo on Joey-PC).

Many sessions share one working tree. Before a session opens SolidWorks to change files in
cad/, or stages cad/ files, it takes the writer lock; it releases it after its commit.
The lock is the gitignored file cad/.cad-writer.lock. The pre-commit hook refuses a commit
touching CAD unless $CAD_WRITER holds the lock. Also hold it while switching branches,
resetting or pulling in the shared checkout.

Usage:
    python tools/cad/cad_lock.py acquire OWNER [--note TEXT] [--take-stale]   take the lock
    python tools/cad/cad_lock.py refresh OWNER                 keep it alive during long work
    python tools/cad/cad_lock.py release OWNER                 give it back
    python tools/cad/cad_lock.py status                        who holds it
    python tools/cad/cad_lock.py check [OWNER]                 exit 0 if OWNER (default $CAD_WRITER)
                                                               holds the lock
OWNER is the thread or person, e.g. "left-arm" or "joey". A lock untouched for 4 hours is
stale: after making sure that session stopped and closed its SolidWorks, take it over with
acquire --take-stale. An unreadable lock file is never stale; inspect and delete it by hand.
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


class Unreadable(dict):
    """A lock file that exists but can't be parsed: treated as HELD (never stale) until removed by hand."""


def read(path: Path = LOCK_PATH) -> dict | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return None
    except (OSError, ValueError):
        return Unreadable(
            owner="(unreadable lock file; check it and delete it by hand)",
            updated=time.time(),
        )


def is_stale(lock: dict, now: float | None = None) -> bool:
    if isinstance(lock, Unreadable):
        return False
    return (now or time.time()) - float(lock.get("updated", 0)) > STALE_SECONDS


def describe(lock: dict) -> str:
    age = (time.time() - float(lock.get("updated", 0))) / 60
    note = f", {lock['note']}" if lock.get("note") else ""
    return f"{lock.get('owner')} on {lock.get('host', '?')}, updated {age:.0f} min ago{note}"


def _tmp_with(path: Path, data: dict) -> Path:
    """Write the full record to a unique temp file beside the lock (never a half-written lock)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f"{path.name}.{os.getpid()}.{time.time_ns()}.tmp")
    tmp.write_text(json.dumps(data), encoding="utf-8")
    return tmp


def _create(path: Path, data: dict) -> bool:
    """Atomically create the lock with its full content; False if it already exists.

    os.link fails if the target exists, so the lock file appears complete or not at all.
    """
    tmp = _tmp_with(path, data)
    try:
        os.link(tmp, path)
        return True
    except FileExistsError:
        return False
    finally:
        tmp.unlink(missing_ok=True)


def _rewrite(path: Path, data: dict) -> None:
    os.replace(
        _tmp_with(path, data), path
    )  # atomic: readers see the old or new record, never empty


def acquire(
    owner: str, note: str = "", path: Path = LOCK_PATH, take_stale: bool = False
) -> tuple[bool, str]:
    now = time.time()
    data = {
        "owner": owner,
        "host": socket.gethostname(),
        "started": now,
        "updated": now,
        "note": note,
    }
    if _create(path, data):
        return True, f"lock taken by {owner}"
    lock = read(path)
    if lock is None:
        return (
            (True, f"lock taken by {owner}")
            if _create(path, data)
            else (False, "another session took the lock first")
        )
    if lock.get("owner") == owner and not isinstance(lock, Unreadable):
        lock["updated"] = now
        _rewrite(path, lock)
        return True, f"{owner} already holds the lock (refreshed)"
    if not is_stale(lock, now):
        return False, f"CAD writer lock held by {describe(lock)}"
    if not take_stale:
        return False, (
            f"stale lock (no refresh for 4 h) held by {describe(lock)}. Make sure that session has stopped "
            "and its SolidWorks is closed, then acquire again with --take-stale."
        )
    return _take_stale(path, lock, data)


def _take_stale(path: Path, stale: dict, data: dict) -> tuple[bool, str]:
    """Replace a stale lock, one recoverer at a time, failing closed.

    The recovery guard is created exclusively and is never removed by anyone but its
    creator (checked by a unique token). If a guard is already there, recovery refuses:
    a guard left by a crash is cleared by hand. Inside the guard the lock is re-read and
    removed only if it is still the same stale record; the new lock is then created
    exclusively, so a normal acquire that slips in between wins and nothing is put back.
    """
    guard = path.with_name(path.name + ".recover")
    token = {
        "pid": os.getpid(),
        "host": socket.gethostname(),
        "token": f"{os.getpid()}-{time.time_ns()}",
    }
    if not _create(guard, token):
        return False, (
            f"recovery guard {guard.name} is present: another session is recovering the lock, "
            "or one crashed while doing so. If no other session is recovering, delete it by hand and retry."
        )
    try:
        if read(path) != stale:
            return False, "the lock changed hands; check status and try again"
        path.unlink(missing_ok=True)
        if _create(path, data):
            return True, f"took over a stale lock from {describe(stale)}"
        return False, "another session took the lock first"
    finally:
        if read(guard) == token:  # only ever remove our own guard
            guard.unlink(missing_ok=True)


def refresh(owner: str, path: Path = LOCK_PATH) -> tuple[bool, str]:
    lock = read(path)
    if not lock or isinstance(lock, Unreadable) or lock.get("owner") != owner:
        return False, f"{owner} does not hold the lock"
    lock["updated"] = time.time()
    _rewrite(path, lock)
    return True, "refreshed"


def release(owner: str, path: Path = LOCK_PATH) -> tuple[bool, str]:
    lock = read(path)
    if not lock:
        return True, "no lock held"
    if isinstance(lock, Unreadable) or lock.get("owner") != owner:
        return False, f"lock belongs to {describe(lock)}, not {owner}"
    path.unlink(missing_ok=True)
    return True, f"released by {owner}"


def check(owner: str | None, path: Path = LOCK_PATH) -> tuple[bool, str]:
    """May OWNER commit CAD? Only while OWNER holds the lock (a stale lock of their own counts)."""
    lock = read(path)
    if (
        owner
        and lock
        and not isinstance(lock, Unreadable)
        and lock.get("owner") == owner
    ):
        return True, f"{owner} holds the lock"
    if not lock:
        return False, (
            "no CAD writer lock held. Take it before changing or committing CAD: "
            "python tools/cad/cad_lock.py acquire <name>, and set CAD_WRITER=<name>."
        )
    if not owner:
        return (
            False,
            f"CAD_WRITER is not set; the lock is held by {describe(lock)}. Set CAD_WRITER=<name> if it is yours.",
        )
    return False, (
        f"CAD writer lock held by {describe(lock)}{' (stale)' if is_stale(lock) else ''}. Wait for it, "
        "or ask that session to release it (python tools/cad/cad_lock.py release OWNER)."
    )


def main(argv: list[str]) -> int:
    if not argv:
        print(__doc__)
        return 2
    cmd, rest = argv[0], argv[1:]
    note = ""
    take_stale = "--take-stale" in rest
    rest = [r for r in rest if r != "--take-stale"]
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
            "acquire": lambda o: acquire(o, note, take_stale=take_stale),
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
