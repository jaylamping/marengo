#!/usr/bin/env python3
"""Check the local CAD tree against the layout rule in cad/manifests/cad-layout.json.

Usage:
    python tools/cad/check_cad_layout.py [CAD_ROOT] [--strict] [--quiet]

CAD_ROOT defaults to the repo's cad/ folder (J:\\code\\marengo\\cad on Joey-PC).
Exit code 0 = clean, 1 = layout errors (or warnings with --strict), 2 = bad usage.
Stdlib only, so it runs on Windows, macOS and Linux without setup.

Errors (break the rule):
  - unknown top-level folder or loose top-level file
  - forbidden folder names anywhere outside _archive (codex, task, task-2, _import*,
    old, solver_results, ...)
  - native SolidWorks parts/assemblies outside a region folder, vendor/, or the
    scratch folders (_wip, _sim, _archive)
  - simulation results (.CWR) outside _sim/ or _archive/
  - a file type that does not belong in a function folder (e.g. SLDPRT in exports/)
  - exports/, prints/, reviews/, drawings/ not split by region
  - date-stamped folders outside reviews/, _wip/, _sim/, _archive/
  - any file of 100 MB or more outside the scratch folders (it would be committed;
    the repo forbids files that large)
Warnings (should be fixed, do not fail without --strict):
  - native file names that do not match cad/manifests/cad-conventions.json
  - version-suffixed copies (_v2, _revC, _final, _copy, ...) of native files in region folders
  - .checkpoints folders (SolidWorks recreates them beside open parts; gitignored)
"""

from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
MAX_FILE_BYTES = 100 * 1000 * 1000  # same limit as tools/check_file_sizes.py
LAYOUT_PATH = REPO_ROOT / "cad" / "manifests" / "cad-layout.json"
CONVENTIONS_PATH = REPO_ROOT / "cad" / "manifests" / "cad-conventions.json"


@dataclass
class Report:
    errors: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)

    def error(self, rel: Path, msg: str) -> None:
        self.errors.append(f"{rel.as_posix()}: {msg}")

    def warn(self, rel: Path, msg: str) -> None:
        self.warnings.append(f"{rel.as_posix()}: {msg}")


def load_json(path: Path) -> dict:
    with path.open(encoding="utf-8") as fh:
        return json.load(fh)


class Checker:
    def __init__(self, root: Path, layout: dict, conventions: dict) -> None:
        self.root = root
        self.layout = layout
        self.regions = {r.lower() for r in layout["regionFolders"]}
        self.functions = {k.lower(): v for k, v in layout["functionFolders"].items()}
        self.scratch = {s.lower() for s in layout["scratchFolders"]}
        self.root_files = {f.lower() for f in layout["rootFiles"]}
        self.native_ext = {e.lower() for e in layout["nativeExtensions"]}
        self.sim_ext = {e.lower() for e in layout["simExtensions"]}
        self.forbidden = [re.compile(p, re.IGNORECASE) for p in layout["forbiddenFolderPatterns"]]
        vs = layout.get("versionSuffixPattern")
        self.version_suffix = re.compile(vs, re.IGNORECASE) if vs else None
        self.warn_dirs = [re.compile(p, re.IGNORECASE) for p in layout.get("warnFolderPatterns", [])]
        self.dated = re.compile(layout["datedFolderPattern"])
        self.dated_ok = {d.lower() for d in layout["datedFoldersAllowedUnder"]}
        self.ignore = [re.compile(p, re.IGNORECASE) for p in layout["ignoreFilePatterns"]]
        flags = re.IGNORECASE
        self.part_re = re.compile(conventions["partFilenamePattern"], flags)
        self.asm_re = re.compile(conventions["assemblyFilenamePattern"], flags)
        self.vendor_re = re.compile(conventions["vendorFilenamePattern"], flags)
        self.report = Report()

    # -- helpers -----------------------------------------------------------
    def ignored(self, name: str) -> bool:
        return any(p.search(name) for p in self.ignore)

    def check_dirs(self, rel_dir: Path) -> None:
        """Folder-name rules (not applied inside _archive)."""
        parts = rel_dir.parts
        if not parts or parts[0].lower() == "_archive":
            return
        name = parts[-1]  # every folder is visited by the walk, so check only its own name
        if any(pat.search(name) for pat in self.forbidden):
            self.report.error(rel_dir, f"forbidden folder name '{name}'")
        if any(pat.search(name) for pat in self.warn_dirs):
            self.report.warn(rel_dir, "SolidWorks checkpoints (gitignored); move to _archive/checkpoints/ when they pile up")
        if self.dated.search(name) and parts[0].lower() not in self.dated_ok:
            self.report.error(rel_dir, "date-stamped folder outside reviews/, _wip/, _sim/, _archive/")

    def check_native_name(self, rel: Path, top: str) -> None:
        name = rel.name
        if top == "vendor":
            if not self.vendor_re.search(name):
                self.report.warn(rel, "vendor file name should be vendor_<mfg>_<mpn>_vendor.*")
            return
        ext = rel.suffix.lower()
        if rel.parent == Path(".") and name.lower() == "marengo.sldasm":
            return
        if ext == ".sldprt" and not self.part_re.search(name):
            self.report.warn(rel, "part name should be marengo_<subsystem>_<part>.SLDPRT")
        if ext == ".sldasm" and not self.asm_re.search(name):
            self.report.warn(rel, "assembly name should be marengo_<subsystem>_asm.SLDASM")

    # -- main walk ---------------------------------------------------------
    def run(self) -> Report:
        for path in sorted(self.root.rglob("*")):
            rel = path.relative_to(self.root)
            if path.is_dir():
                self.check_top_dir(rel)
                self.check_dirs(rel)
                continue
            if self.ignored(path.name):
                continue
            self.check_file(rel)
        return self.report

    def check_top_dir(self, rel: Path) -> None:
        if len(rel.parts) != 1:
            return
        name = rel.parts[0].lower()
        if name in self.regions or name in self.functions or name in self.scratch:
            return
        self.report.error(rel, "unknown top-level folder (not a region, function or scratch folder)")

    def check_file(self, rel: Path) -> None:
        parts = rel.parts
        ext = rel.suffix.lower()
        if len(parts) == 1:
            if rel.name.lower() not in self.root_files:
                self.report.error(rel, "loose file at CAD root (only README.md and marengo.SLDASM belong here)")
            return
        top = parts[0].lower()
        if top not in self.scratch:
            size = (self.root / rel).stat().st_size
            if size >= MAX_FILE_BYTES:
                self.report.error(
                    rel, f"{size / 1e6:.1f} MB: files of 100 MB or more can't go in git; move to _wip/ or _archive/"
                )
        if top in self.scratch:
            if ext in self.sim_ext and top == "_wip":
                self.report.error(rel, "simulation results belong in _sim/")
            return
        if ext in self.sim_ext:
            self.report.error(rel, "simulation results belong in _sim/<region>/")
            return
        if top in self.regions:
            if ext in self.native_ext:
                self.check_native_name(rel, top)
                if self.version_suffix and self.version_suffix.search(rel.name):
                    self.report.warn(
                        rel, "version-suffixed copy in a region folder; keep one current file, history is in git"
                    )
            return
        if top in self.functions:
            self.check_function_file(rel, top, ext)
            return
        # unknown top-level folder: already reported once on the folder itself

    def check_function_file(self, rel: Path, top: str, ext: str) -> None:
        rule = self.functions[top]
        parts = rel.parts
        if ext in self.native_ext and top != "vendor":
            self.report.error(rel, f"native SolidWorks file in {top}/ (move to a region folder or _wip/)")
            return
        if top == "vendor" and ext in self.native_ext:
            self.check_native_name(rel, top)
        allowed = rule.get("extensions")
        if allowed is not None and ext not in {e.lower() for e in allowed}:
            self.report.error(rel, f"'{ext or '(none)'}' files do not belong in {top}/")
        if rule.get("regionSubfolders"):
            if len(parts) < 3:
                self.report.error(rel, f"{top}/ must be split by region: {top}/<region>/...")
            elif parts[1].lower() not in self.regions:
                self.report.error(rel, f"'{parts[1]}' is not a region folder name")
        if rule.get("datedSubfolders") and len(parts) >= 4 and not self.dated.search(parts[2]):
            self.report.error(rel, f"{top}/<region>/ subfolders are named <YYYYMMDD>_<topic>")


def main(argv: list[str]) -> int:
    args = [a for a in argv if not a.startswith("--")]
    flags = {a for a in argv if a.startswith("--")}
    unknown = flags - {"--strict", "--quiet"}
    if unknown or len(args) > 1:
        print(__doc__)
        return 2
    root = Path(args[0]) if args else REPO_ROOT / "cad"
    if not root.is_dir():
        print(f"CAD root not found: {root}")
        return 2
    checker = Checker(root, load_json(LAYOUT_PATH), load_json(CONVENTIONS_PATH))
    report = checker.run()
    quiet = "--quiet" in flags
    for line in report.errors:
        print(f"ERROR {line}")
    if not quiet:
        for line in report.warnings:
            print(f"WARN  {line}")
    print(f"\n{len(report.errors)} error(s), {len(report.warnings)} warning(s) in {root}")
    failed = bool(report.errors) or ("--strict" in flags and bool(report.warnings))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
