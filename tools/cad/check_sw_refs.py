#!/usr/bin/env python3
"""Promotion check (Windows, SolidWorks): every file an assembly or part references resolves
to a file under cad/ that git will hold (not a scratch folder, not outside cad/).

    python tools/cad/check_sw_refs.py cad\\arm\\marengo_arm_left_asm.SLDASM [more files...]
    python tools/cad/check_sw_refs.py --all      every SLDPRT/SLDASM/SLDDRW in cad/ outside the scratch folders

Run it after staging a promoted part or assembly (git add), before committing: every
reference must be committed or staged. Reads references with
ISldWorks.GetDocumentDependencies2 (no document is opened) in this session's own hidden
SolidWorks instance (hardware/tools/solidworks/sw_instance.py, see RULES.md).
Exit code 0 = every reference resolves inside tracked cad/, 1 = a broken or outside reference.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path, PureWindowsPath

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gitlfs import REPO_ROOT, git

CAD_ROOT = REPO_ROOT / "cad"
SCRATCH = {"_wip", "_sim", "_archive", ".checkpoints"}


def classify(
    ref: str,
    cad_root: Path = CAD_ROOT,
    ignored: set[str] | None = None,
    indexed: set[str] | None = None,
) -> str | None:
    """None if the reference is fine, else why not. `ignored` = lower-case cad-relative paths git ignores."""
    p = Path(ref)
    if not p.is_file():
        return "missing (SolidWorks would search for it or show it suppressed)"
    try:
        rel = p.resolve().relative_to(cad_root.resolve())
    except ValueError:
        return "outside cad/ (copy it in with Pack and Go)"
    if SCRATCH & {part.lower() for part in rel.parts[:-1]}:
        return "in a scratch folder (_wip/_sim/_archive are not committed)"
    if ignored and rel.as_posix().lower() in ignored:
        return "gitignored, so it would not be committed"
    if indexed is not None and rel.as_posix().lower() not in indexed:
        return "not in git yet (neither committed nor staged); git add it with the promotion"
    return None


def git_ignored(paths: list[Path]) -> set[str]:
    rels = []
    for p in paths:
        try:
            rels.append(p.resolve().relative_to(REPO_ROOT.resolve()).as_posix())
        except ValueError:
            pass
    if not rels:
        return set()
    try:
        out = git(
            "check-ignore",
            "--no-index",
            "--stdin",
            input_bytes=("\n".join(rels) + "\n").encode(),
        )
    except subprocess.CalledProcessError:  # exit 1 = nothing ignored
        return set()
    return {
        ln[len("cad/") :].lower()
        for ln in out.decode().splitlines()
        if ln.startswith("cad/")
    }


def unstaged_paths() -> set[str]:
    """Lower-case cad-relative paths whose working copy differs from the staged copy.

    The scan reads working-tree files, so it only proves what gets committed when those match.
    """
    out = git("diff", "--name-only", "-z", "--", "cad").decode()
    return {p[len("cad/") :].lower() for p in out.split("\0") if p.startswith("cad/")}


def cad_key(path: Path) -> str | None:
    try:
        return path.resolve().relative_to(CAD_ROOT.resolve()).as_posix().lower()
    except ValueError:
        return None


def index_paths() -> set[str]:
    """Lower-case cad-relative paths in the index (committed or staged)."""
    out = git("ls-files", "-z", "--", "cad").decode()
    return {p[len("cad/") :].lower() for p in out.split("\0") if p.startswith("cad/")}


def dependencies(sw, path: Path) -> list[str]:
    # (file, traverse all levels, search for files, add read-only info) -> [name, path, name, path, ...]
    # search=False: report the stored paths, so a reference SolidWorks would only find by searching shows up as missing
    deps = sw.GetDocumentDependencies2(str(path), True, False, False) or ()
    return [deps[i + 1] for i in range(0, len(deps) - 1, 2)]


def targets(argv: list[str]) -> list[Path]:
    """--all: every staged or committed native file in cad/ outside the scratch folders."""
    if argv == ["--all"]:
        native = {".sldprt", ".sldasm", ".slddrw"}
        out = git("ls-files", "-z", "--", "cad").decode()
        found = []
        for rel in sorted(p for p in out.split("\0") if p):
            parts = {x.lower() for x in Path(rel).parts}
            if Path(rel).suffix.lower() in native and not (SCRATCH & parts):
                found.append(REPO_ROOT / rel)
        return found
    return [Path(a).resolve() for a in argv]


def main(argv: list[str]) -> int:
    if not argv or argv in (["-h"], ["--help"]):
        print(__doc__)
        return 2
    sys.path.insert(0, str(REPO_ROOT / "hardware" / "tools" / "solidworks"))
    from sw_instance import launch  # Windows only (pywin32)

    indexed = index_paths()
    files = targets(argv)
    bad = 0
    unstaged = unstaged_paths()
    with launch() as inst:
        for f in files:
            key = cad_key(f)
            if key is None or key not in indexed:
                bad += 1
                print(f"BAD  {f}: not staged or committed under cad/; git add it first")
                continue
            if key in unstaged:
                bad += 1
                print(
                    f"BAD  {f.relative_to(REPO_ROOT)}: working copy differs from the staged copy; stage it (git add) or restore it, then re-run"
                )
                continue
            refs = dependencies(inst.sw, f)
            for ref in refs:
                rkey = cad_key(Path(ref))
                if rkey and rkey in unstaged:
                    bad += 1
                    print(
                        f"BAD  {f.relative_to(REPO_ROOT)} -> {PureWindowsPath(ref)}: working copy differs from the staged copy; stage it or restore it"
                    )
            ignored = git_ignored([Path(r) for r in refs])
            for ref in refs:
                why = classify(ref, ignored=ignored, indexed=indexed)
                if why:
                    bad += 1
                    print(
                        f"BAD  {f.relative_to(REPO_ROOT)} -> {PureWindowsPath(ref)}: {why}"
                    )
            print(f"ok   {f.relative_to(REPO_ROOT)}: {len(refs)} reference(s) checked")
    print(f"\n{bad} bad reference(s) in {len(files)} file(s)")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
