#!/usr/bin/env python3
"""Check the CAD tree against the layout rule in cad/manifests/cad-layout.json.

Usage:
    python tools/cad/check_cad_layout.py [CAD_ROOT] [--strict] [--quiet]
    python tools/cad/check_cad_layout.py --repo [--strict] [--quiet]

CAD_ROOT defaults to the repo's cad/ folder (J:\\code\\marengo\\cad on Joey-PC).
--repo checks what git tracks instead of a folder: CAD files outside cad/, tracked files in
scratch folders, and the tracked part of cad/ (works in a clone without LFS content).
Exit code 0 = clean, 1 = layout errors (or warnings with --strict), 2 = bad usage.
Stdlib only, so it runs on Windows, macOS and Linux without setup.

Errors (break the rule):
  - unknown top-level folder or loose top-level file
  - forbidden folder names anywhere outside _archive (codex, task, task-2, _import*, old, ...)
  - native SolidWorks files outside a region folder, vendor/, or the scratch folders
  - a file type that does not belong in its folder (region folders hold SLDPRT/SLDASM/md;
    arm/ holds whole-arm assemblies only; SLDPRT in exports/; STEP in prints/; ...)
  - simulation results (.CWR, .STE) outside _sim/ or _archive/
  - exports/, prints/, reviews/, drawings/ not split by region; reviews/ not split into
    reviews/<region>/<YYYYMMDD>_<topic>/
  - date-stamped folders outside reviews/, _wip/, _sim/, _archive/
  - version-suffixed copies (_v2, _revC, _final, _copy, "Copy of", ...) in region, exports,
    prints and drawings folders: keep one current file, history is in git
  - any file of 100 MB or more outside the scratch folders (real size, LFS pointers included)
  - a review file over 10 MB
  - an export whose bytes no longer match cad/manifests/exports.json, or whose source is gone
  - --repo: a CAD file tracked outside cad/, or any tracked file under _wip/_sim/_archive/.checkpoints
Warnings (should be fixed, do not fail without --strict):
  - native file names that do not match cad/manifests/cad-conventions.json
  - .checkpoints folders (SolidWorks recreates them beside open parts; gitignored)
  - stale exports (source changed since the export was recorded) and exports with no record
  - review files over 2 MB, packets over 15 MB or 10 images, packets with no pinned commit
Held: paths listed in cad/manifests/layout-exceptions.txt report their errors as "HELD"
  (shown, not failing) until they move.
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path, PurePosixPath

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gitlfs import (
    MAX_FILE_BYTES,
    REPO_ROOT,
    blob_sizes,
    content_sha256,
    git,
    parse_pointer,
    real_blob_size,
    real_size,
)

LAYOUT_PATH = REPO_ROOT / "cad" / "manifests" / "cad-layout.json"
CONVENTIONS_PATH = REPO_ROOT / "cad" / "manifests" / "cad-conventions.json"
EXPORTS_PATH = REPO_ROOT / "cad" / "manifests" / "exports.json"
EXCEPTIONS_PATH = REPO_ROOT / "cad" / "manifests" / "layout-exceptions.txt"
IMAGE_EXT = {".png", ".jpg", ".jpeg", ".svg"}


@dataclass
class Report:
    errors: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)
    held: list[str] = field(default_factory=list)
    prefix: str = ""  # repo-relative prefix of the checked root, for exception matching
    exceptions: list[str] = field(default_factory=list)

    def _is_held(self, rel: str) -> bool:
        full = self.prefix + rel
        return any(full == e.rstrip("/") or full.startswith(e) for e in self.exceptions)

    def error(self, rel: PurePosixPath | Path | str, msg: str) -> None:
        r = rel if isinstance(rel, str) else PurePosixPath(*Path(rel).parts).as_posix()
        (self.held if self._is_held(r) else self.errors).append(f"{r}: {msg}")

    def warn(self, rel: PurePosixPath | Path | str, msg: str) -> None:
        r = rel if isinstance(rel, str) else PurePosixPath(*Path(rel).parts).as_posix()
        self.warnings.append(f"{r}: {msg}")


def load_json(path: Path) -> dict:
    with path.open(encoding="utf-8") as fh:
        return json.load(fh)


def parse_exceptions(text: str) -> list[str]:
    lines = (ln.strip() for ln in text.splitlines())
    return [ln for ln in lines if ln and not ln.startswith("#")]


def load_exceptions(path: Path = EXCEPTIONS_PATH) -> list[str]:
    return parse_exceptions(path.read_text(encoding="utf-8")) if path.exists() else []


class Checker:
    """Checks a list of (path relative to the CAD root, real size). Folders come from the paths."""

    def __init__(
        self,
        root: Path,
        layout: dict,
        conventions: dict,
        exports: dict | None = None,
        exceptions: list[str] | None = None,
        prefix: str = "cad/",
        source: FileSource | None = None,
    ) -> None:
        self.root = root
        self.source = source or DiskSource(root)
        self.layout = layout
        low = lambda xs: {x.lower() for x in xs}
        self.regions = low(layout["regionFolders"])
        self.region_ext = low(layout["regionExtensions"])
        self.region_ext_override = {
            k.lower(): low(v)
            for k, v in layout.get("regionExtensionOverrides", {}).items()
        }
        self.functions = {k.lower(): v for k, v in layout["functionFolders"].items()}
        self.scratch = low(layout["scratchFolders"])
        self.root_files = low(layout["rootFiles"])
        self.native_ext = low(layout["nativeExtensions"])
        self.sim_ext = low(layout["simExtensions"])
        self.forbidden = [
            re.compile(p, re.IGNORECASE) for p in layout["forbiddenFolderPatterns"]
        ]
        self.version_suffix = re.compile(layout["versionSuffixPattern"], re.IGNORECASE)
        vs_folders = low(layout["versionSuffixFolders"])
        self.version_suffix_tops = (
            self.regions if "regions" in vs_folders else set()
        ) | (vs_folders - {"regions"})
        self.warn_dirs = [
            re.compile(p, re.IGNORECASE) for p in layout.get("warnFolderPatterns", [])
        ]
        self.dated = re.compile(layout["datedFolderPattern"])
        self.dated_ok = low(layout["datedFoldersAllowedUnder"])
        self.review_dir = re.compile(layout["reviewFolderPattern"])
        self.review_limits = layout["reviewLimits"]
        self.ignore = [
            re.compile(p, re.IGNORECASE) for p in layout["ignoreFilePatterns"]
        ]
        self.part_re = re.compile(conventions["partFilenamePattern"], re.IGNORECASE)
        self.asm_re = re.compile(conventions["assemblyFilenamePattern"], re.IGNORECASE)
        self.vendor_re = re.compile(conventions["vendorFilenamePattern"], re.IGNORECASE)
        self.exports = exports
        self.report = Report(prefix=prefix, exceptions=exceptions or [])
        self.packets: dict[str, list[tuple[PurePosixPath, int]]] = defaultdict(list)

    # -- entry points --------------------------------------------------------
    def run(self) -> Report:
        """Walk the folder on disk."""
        files = []
        for path in sorted(self.root.rglob("*")):
            if path.is_file():
                files.append(
                    (PurePosixPath(*path.relative_to(self.root).parts), real_size(path))
                )
        return self.check(files)

    def check(self, files: list[tuple[PurePosixPath, int]]) -> Report:
        dirs = set()
        for rel, _ in files:
            for i in range(1, len(rel.parts)):
                dirs.add(PurePosixPath(*rel.parts[:i]))
        for d in sorted(dirs):
            self.check_top_dir(d)
            self.check_dir_name(d)
        for rel, size in sorted(files):
            if not self.ignored(rel.name):
                self.check_file(rel, size)
        self.check_packets()
        if self.exports is not None:
            self.check_exports({rel.as_posix().lower() for rel, _ in files})
        return self.report

    # -- helpers -------------------------------------------------------------
    def ignored(self, name: str) -> bool:
        return any(p.search(name) for p in self.ignore)

    def check_top_dir(self, rel: PurePosixPath) -> None:
        if len(rel.parts) != 1:
            return
        name = rel.parts[0].lower()
        if name in self.regions or name in self.functions or name in self.scratch:
            return
        self.report.error(
            rel, "unknown top-level folder (not a region, function or scratch folder)"
        )

    def check_dir_name(self, rel: PurePosixPath) -> None:
        """Folder-name rules (not applied inside _archive)."""
        parts = rel.parts
        if parts[0].lower() == "_archive":
            return
        name = parts[-1]
        if any(pat.search(name) for pat in self.forbidden):
            self.report.error(rel, f"forbidden folder name '{name}'")
        if any(pat.search(name) for pat in self.warn_dirs):
            self.report.warn(
                rel,
                "SolidWorks checkpoints (gitignored); move to _archive/checkpoints/ when they pile up",
            )
        if self.dated.search(name) and parts[0].lower() not in self.dated_ok:
            self.report.error(
                rel, "date-stamped folder outside reviews/, _wip/, _sim/, _archive/"
            )

    def check_native_name(self, rel: PurePosixPath, top: str) -> None:
        name = rel.name
        if top == "vendor":
            if not self.vendor_re.search(name):
                self.report.warn(
                    rel, "vendor file name should be vendor_<mfg>_<mpn>_vendor.*"
                )
            return
        ext = rel.suffix.lower()
        if ext == ".sldprt" and not self.part_re.search(name):
            self.report.warn(
                rel, "part name should be marengo_<subsystem>_<part>.SLDPRT"
            )
        if ext == ".sldasm" and not self.asm_re.search(name):
            self.report.warn(
                rel, "assembly name should be marengo_<subsystem>_asm.SLDASM"
            )

    # -- files ---------------------------------------------------------------
    def check_file(self, rel: PurePosixPath, size: int) -> None:
        parts = rel.parts
        ext = rel.suffix.lower()
        top = parts[0].lower()
        if top not in self.scratch and size >= MAX_FILE_BYTES:
            self.report.error(
                rel,
                f"{size / 1e6:.1f} MB: files of 100 MB or more can't go in git; move to _wip/ or _archive/",
            )
        if len(parts) == 1:
            if rel.name.lower() not in self.root_files:
                self.report.error(
                    rel,
                    "loose file at CAD root (only README.md and marengo.SLDASM belong here)",
                )
            return
        if top in self.scratch:
            if ext in self.sim_ext and top == "_wip":
                self.report.error(rel, "simulation results belong in _sim/")
            return
        if ext in self.sim_ext:
            self.report.error(rel, "simulation results belong in _sim/<region>/")
            return
        if top in self.version_suffix_tops and self.version_suffix.search(rel.name):
            self.report.error(
                rel,
                "version-suffixed copy; keep one current file and overwrite it, history is in git",
            )
        if top in self.regions:
            allowed = self.region_ext_override.get(top, self.region_ext)
            if ext not in allowed:
                what = (
                    "whole-arm assemblies only"
                    if top == "arm"
                    else "native parts and assemblies"
                )
                self.report.error(
                    rel, f"'{ext or '(none)'}' files do not belong in {top}/ ({what})"
                )
            elif ext in self.native_ext:
                self.check_native_name(rel, top)
            return
        if top in self.functions:
            self.check_function_file(rel, top, ext, size)

    def check_function_file(
        self, rel: PurePosixPath, top: str, ext: str, size: int
    ) -> None:
        rule = self.functions[top]
        parts = rel.parts
        if ext in self.native_ext and top != "vendor":
            self.report.error(
                rel,
                f"native SolidWorks file in {top}/ (move to a region folder or _wip/)",
            )
            return
        if top == "vendor" and ext in self.native_ext:
            self.check_native_name(rel, top)
        allowed = rule.get("extensions")
        if allowed is not None and ext not in {e.lower() for e in allowed}:
            self.report.error(rel, f"'{ext or '(none)'}' files do not belong in {top}/")
        if rule.get("regionSubfolders"):
            if len(parts) < 3:
                self.report.error(
                    rel, f"{top}/ must be split by region: {top}/<region>/..."
                )
                return
            if parts[1].lower() not in self.regions:
                self.report.error(rel, f"'{parts[1]}' is not a region folder name")
        if rule.get("datedSubfolders"):
            if len(parts) < 4:
                self.report.error(
                    rel, f"{top}/ files go in {top}/<region>/<YYYYMMDD>_<topic>/"
                )
                return
            if not self.review_dir.search(parts[2]):
                self.report.error(
                    rel,
                    f"'{parts[2]}' should be named <YYYYMMDD>_<topic> (lower case, hyphens)",
                )
            if top == "reviews":
                self.check_review_file(rel, size)

    def check_review_file(self, rel: PurePosixPath, size: int) -> None:
        lim = self.review_limits
        if size > lim["fileMaxBytes"]:
            self.report.error(
                rel,
                f"{size / 1e6:.1f} MB review file; keep bulky data in _archive/reviews/",
            )
        elif size > lim["fileWarnBytes"]:
            self.report.warn(
                rel,
                f"{size / 1e6:.1f} MB review file; keep renders under 2 MB (about 1600 px)",
            )
        self.packets["/".join(rel.parts[:3])].append((rel, size))

    def check_packets(self) -> None:
        lim = self.review_limits
        pinned = re.compile(lim["pinnedCommitPattern"])
        for packet, files in sorted(self.packets.items()):
            total = sum(s for _, s in files)
            images = sum(1 for r, _ in files if r.suffix.lower() in IMAGE_EXT)
            if total > lim["packetWarnBytes"]:
                self.report.warn(
                    packet,
                    f"review packet is {total / 1e6:.1f} MB; keep a packet under 15 MB",
                )
            if images > lim["packetImagesWarn"]:
                self.report.warn(
                    packet, f"{images} images; commit the key renders (10 or fewer)"
                )
            if not any(
                self._has_pin(r, pinned) for r, _ in files if r.suffix.lower() == ".md"
            ):
                self.report.warn(
                    packet,
                    "no 'Pinned commit: <full 40-character SHA>' line in the packet's markdown",
                )

    def _has_pin(self, rel: PurePosixPath, pinned: re.Pattern) -> bool:
        data = self.source.read(rel.as_posix())
        return bool(data and pinned.search(data.decode("utf-8", errors="replace")))

    # -- exports manifest ----------------------------------------------------
    def check_exports(self, present: set[str]) -> None:
        recorded = set()
        for entry in self.exports.get("exports", []):
            exp, src = entry["export"], entry["source"]
            recorded.add(exp.lower())
            exp_sha, src_sha = self.source.sha256(exp), self.source.sha256(src)
            if exp_sha is None:
                self.report.error(exp, "listed in manifests/exports.json but missing")
                continue
            if exp_sha != entry["exportSha256"]:
                self.report.error(
                    exp,
                    "changed since it was recorded; run tools/cad/record_export.py again",
                )
            if src_sha is None:
                self.report.error(
                    exp,
                    f"source {src} is gone; re-export from the current part or delete the export",
                )
            elif src_sha != entry["sourceSha256"]:
                self.report.warn(
                    exp,
                    f"stale: {src} changed after this export; re-export and record it",
                )
        for rel in sorted(present):
            p = PurePosixPath(rel)
            unrecorded = (
                p.parts[0] == "exports"
                and p.suffix in {".step", ".stp", ".stl"}
                and rel not in recorded
            )
            if unrecorded and not self.report._is_held(rel):
                self.report.warn(
                    rel,
                    "no source recorded; run tools/cad/record_export.py EXPORT SOURCE",
                )


# -- where file contents come from -------------------------------------------
class DiskSource:
    """Files on disk under the CAD root."""

    def __init__(self, root: Path) -> None:
        self.root = root

    def read(self, rel: str) -> bytes | None:
        try:
            return (self.root / rel).read_bytes()
        except OSError:
            return None

    def sha256(self, rel: str) -> str | None:
        path = self.root / rel
        return content_sha256(path) if path.is_file() else None


class IndexSource:
    """The staged (index) copy of files under cad/, i.e. exactly what a commit holds."""

    def __init__(self, blobs: dict[str, str], root: Path = REPO_ROOT) -> None:
        self.blobs = {
            k.lower(): v for k, v in blobs.items()
        }  # cad-relative path -> blob id
        self.root = root

    def read(self, rel: str) -> bytes | None:
        blob = self.blobs.get(rel.lower())
        return git("cat-file", "blob", blob, root=self.root) if blob else None

    def sha256(self, rel: str) -> str | None:
        data = self.read(rel)
        if data is None:
            return None
        ptr = parse_pointer(data) if len(data) <= 1024 else None
        return ptr[0] if ptr else hashlib.sha256(data).hexdigest()


FileSource = DiskSource | IndexSource


# -- repo mode ---------------------------------------------------------------
def tracked_files(root: Path = REPO_ROOT) -> list[tuple[str, str]]:
    """(blob id, path) for everything in the index."""
    out = []
    for line in git("ls-files", "-s", "-z", root=root).decode().split("\0"):
        if line:
            meta, rel = line.split("\t", 1)
            out.append((meta.split()[1], rel))
    return out


def check_repo(layout: dict, conventions: dict, root: Path = REPO_ROOT) -> Report:
    entries = tracked_files(root)
    sizes = blob_sizes([b for b, _ in entries], root=root)
    # the staged copy of the exceptions list, so an unstaged edit can't hold a file
    exc_blob = next(
        (b for b, r in entries if r == "cad/manifests/layout-exceptions.txt"), None
    )
    exc_text = (
        git("cat-file", "blob", exc_blob, root=root).decode("utf-8") if exc_blob else ""
    )
    exceptions = parse_exceptions(exc_text)
    cad_files = []
    report = Report(prefix="", exceptions=exceptions)
    cad_ext = {e.lower() for e in layout["cadExtensions"]}
    scratch = {s.lower() for s in layout["scratchFolders"]} | {".checkpoints"}
    never_tracked = re.compile(
        r"(^~\$|Zone\.Identifier$|^\.cad-writer\.lock$)", re.IGNORECASE
    )
    cad_blobs = {}
    for blob, rel in entries:
        p = PurePosixPath(rel)
        if never_tracked.search(p.name):
            report.error(
                rel,
                "lock or download-marker file must not be tracked; git rm --cached it",
            )
            continue
        if scratch & {part.lower() for part in p.parts[:-1]}:
            report.error(
                rel,
                "tracked file in a scratch folder (_wip/_sim/_archive/.checkpoints are local only); git rm --cached it",
            )
            continue
        if p.parts[0] == "cad":
            cad_blobs[PurePosixPath(*p.parts[1:]).as_posix()] = blob
            entry = sizes.get(blob)
            if p.suffix.lower() in cad_ext and entry is not None and entry[1] is None:
                report.error(
                    rel,
                    "CAD file committed as a plain git blob, not through Git LFS (git lfs install, then re-add it)",
                )
            cad_files.append(
                (PurePosixPath(*p.parts[1:]), real_blob_size(sizes.get(blob)))
            )
        elif p.suffix.lower() in cad_ext:
            report.error(
                rel,
                "CAD file outside cad/ (Joey 2026-11-08: CAD files live only in cad/; docs link there)",
            )
    source = IndexSource(cad_blobs, root)
    manifest = source.read("manifests/exports.json")
    exports = json.loads(manifest) if manifest else None
    checker = Checker(
        root / "cad",
        layout,
        conventions,
        exports=exports,
        exceptions=exceptions,
        prefix="cad/",
        source=source,
    )
    sub = checker.check(cad_files)
    report.errors += [f"cad/{e}" for e in sub.errors]
    report.warnings += [f"cad/{w}" for w in sub.warnings]
    report.held += [f"cad/{h}" for h in sub.held]
    return report


def group_held(report: Report) -> list[str]:
    """One line per exception entry: the held paths are listed in layout-exceptions.txt."""
    counts: dict[str, int] = defaultdict(int)
    for line in report.held:
        rel = line.split(": ", 1)[0]
        full = (
            rel
            if rel.startswith("cad/") or report.prefix == ""
            else report.prefix + rel
        )
        key = next(
            (
                e
                for e in report.exceptions
                if full.startswith(e) or full == e.rstrip("/")
            ),
            rel,
        )
        counts[key] += 1
    return [
        f"{k}: {n} file(s) still breaking the layout (listed in manifests/layout-exceptions.txt)"
        for k, n in sorted(counts.items())
    ]


def main(argv: list[str]) -> int:
    args = [a for a in argv if not a.startswith("--")]
    flags = {a for a in argv if a.startswith("--")}
    unknown = flags - {"--strict", "--quiet", "--repo"}
    if unknown or len(args) > 1 or ("--repo" in flags and args):
        print(__doc__)
        return 2
    layout, conventions = load_json(LAYOUT_PATH), load_json(CONVENTIONS_PATH)
    if "--repo" in flags:
        report, where = check_repo(layout, conventions), "tracked files"
    else:
        root = Path(args[0]) if args else REPO_ROOT / "cad"
        if not root.is_dir():
            print(f"CAD root not found: {root}")
            return 2
        exports = load_json(EXPORTS_PATH) if EXPORTS_PATH.exists() else None
        report = Checker(
            root, layout, conventions, exports=exports, exceptions=load_exceptions()
        ).run()
        where = str(root)
    quiet = "--quiet" in flags
    for line in report.errors:
        print(f"ERROR {line}")
    if not quiet:
        for line in report.warnings:
            print(f"WARN  {line}")
        for line in group_held(report):
            print(f"HELD  {line}")
    print(
        f"\n{len(report.errors)} error(s), {len(report.warnings)} warning(s), {len(report.held)} held in {where}"
    )
    failed = bool(report.errors) or ("--strict" in flags and bool(report.warnings))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
