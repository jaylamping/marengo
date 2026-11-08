"""Small git + Git LFS helpers shared by the CAD tools (stdlib only).

Sizes are always the REAL file size: for an LFS pointer (working tree, index or
history) the size is read from the pointer, so every check works in a checkout
without LFS content (CI, cloud sessions) as well as on Joey-PC.
"""

from __future__ import annotations

import hashlib
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
LFS_HEADER = b"version https://git-lfs.github.com/spec/v1"
MAX_FILE_BYTES = 100 * 1000 * 1000  # Joey's rule: no file of 100 MB or more


def git(*args: str, root: Path = REPO_ROOT, input_bytes: bytes | None = None) -> bytes:
    # safe.directory: CI runs the checkout as a different user inside Docker.
    out = subprocess.run(
        ["git", "-c", f"safe.directory={root}", "-C", str(root), *args],
        input=input_bytes,
        capture_output=True,
        check=True,
    )
    return out.stdout


def parse_pointer(data: bytes) -> tuple[str, int] | None:
    """Return (sha256 oid, size) if data is an LFS pointer, else None."""
    if not data.startswith(LFS_HEADER) or len(data) > 1024:
        return None
    oid, size = None, None
    for line in data.splitlines():
        if line.startswith(b"oid sha256:"):
            oid = line.split(b":", 1)[1].strip().decode()
        elif line.startswith(b"size "):
            size = int(line.split()[1])
    if oid is None or size is None:
        return None
    return oid, size


def real_size(path: Path) -> int:
    size = path.stat().st_size
    if size <= 1024:
        ptr = parse_pointer(path.read_bytes())
        if ptr:
            return ptr[1]
    return size


def content_sha256(path: Path) -> str:
    """sha256 of the real content; for an LFS pointer this is its oid (same value)."""
    if path.stat().st_size <= 1024:
        ptr = parse_pointer(path.read_bytes())
        if ptr:
            return ptr[0]
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def blob_sizes(
    oids: list[str], root: Path = REPO_ROOT
) -> dict[str, tuple[int, tuple[str, int] | None]]:
    """For git blob ids: (stored size, LFS pointer or None). Missing or non-blob ids are left out.

    Sizes come from a cheap batch-check; only blobs small enough to be a pointer are read.
    """
    oids = sorted(set(oids))
    if not oids:
        return {}
    out = git(
        "cat-file",
        "--batch-check=%(objectname) %(objecttype) %(objectsize)",
        root=root,
        input_bytes=("\n".join(oids) + "\n").encode(),
    )
    result: dict[str, tuple[int, tuple[str, int] | None]] = {}
    small = []
    for line in out.decode().splitlines():
        parts = line.split()
        if len(parts) == 3 and parts[1] == "blob":
            size = int(parts[2])
            result[parts[0]] = (size, None)
            if size <= 1024:
                small.append(parts[0])
    if small:
        body = git(
            "cat-file",
            "--batch",
            root=root,
            input_bytes=("\n".join(small) + "\n").encode(),
        )
        pos = 0
        for oid in small:
            nl = body.index(b"\n", pos)
            size = int(body[pos:nl].split()[2])
            data = body[nl + 1 : nl + 1 + size]
            pos = nl + 1 + size + 1
            result[oid] = (size, parse_pointer(data))
    return result


def real_blob_size(entry: tuple[int, tuple[str, int] | None] | None) -> int:
    """Real size of a blob_sizes() entry: the pointer's size for LFS, else the stored size."""
    if entry is None:
        return 0
    size, ptr = entry
    return ptr[1] if ptr else size


def changed_blobs(rev_args: list[str], root: Path = REPO_ROOT) -> list[tuple[str, str]]:
    """(blob id, path) for every file added or changed by the commits rev-list selects."""
    found = []
    revs = git("rev-list", *rev_args, root=root).decode().split()
    for rev in revs:
        tree = git(
            "diff-tree",
            "-m",  # merges too: content created in a merge is compared against each parent
            "-r",
            "-z",
            "--no-commit-id",
            "--diff-filter=ACMR",
            "--root",
            rev,
            root=root,
        ).decode()
        parts = [p for p in tree.split("\0") if p]
        # ":<old mode> <new mode> <old blob> <new blob> <status>" then the path (renames: two paths)
        i = 0
        while i < len(parts):
            meta = parts[i].split()
            status = meta[4]
            n_paths = 2 if status[0] in "RC" else 1
            found.append((meta[3], parts[i + n_paths]))
            i += 1 + n_paths
    return found


ZERO = "0" * 40


def pre_push_rev_args(stdin_text: str, remote: str) -> list[str]:
    """One rev-list argument list for ALL commits a push sends (every ref together), from the
    pre-push hook's stdin. Empty when the push sends nothing (only deletes)."""
    tips, known = [], []
    for line in stdin_text.splitlines():
        parts = line.split()
        if len(parts) != 4:
            continue
        _local_ref, local_sha, _remote_ref, remote_sha = parts
        if local_sha == ZERO:
            continue  # deleting a remote branch sends nothing
        tips.append(local_sha)
        if remote_sha != ZERO:
            known.append(f"^{remote_sha}")
    if not tips:
        return []
    return [*tips, *known, "--not", f"--remotes={remote}"]
