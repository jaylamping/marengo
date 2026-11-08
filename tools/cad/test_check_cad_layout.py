"""Tests for the CAD tools in tools/cad and tools/check_file_sizes.py (stdlib unittest; run by scripts/check.sh)."""

import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import cad_lock
import check_cad_layout as ccl
import check_file_sizes as cfs
import check_sw_refs
import gitlfs
import install_hooks
import lfs_budget
import record_export

PIN = "Pinned commit: " + "a" * 40 + "\n"


def build(root: Path, files) -> None:
    """files: list of paths (empty files) or dict path -> text content."""
    items = files.items() if isinstance(files, dict) else ((f, "") for f in files)
    for rel, text in items:
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8")


def checker(root: Path, exports=None, exceptions=None) -> ccl.Checker:
    return ccl.Checker(
        root,
        ccl.load_json(ccl.LAYOUT_PATH),
        ccl.load_json(ccl.CONVENTIONS_PATH),
        exports=exports,
        exceptions=exceptions,
    )


def run(files, exceptions=None) -> ccl.Report:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        build(root, files)
        return checker(root, exceptions=exceptions).run()


def pointer(size: int, oid: str = "b" * 64) -> str:
    return (
        f"version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {size}\n"
    )


class LayoutTests(unittest.TestCase):
    def test_clean_tree(self) -> None:
        report = run(
            {
                "README.md": "",
                "marengo.SLDASM": "",
                "shoulder/marengo_shoulder_pitch_bracket_left.SLDPRT": "",
                "arm/marengo_arm_left_asm.SLDASM": "",
                "vendor/robstride/rs03/vendor_robstride_rs03_vendor.SLDPRT": "",
                "exports/shoulder/marengo_shoulder_pitch_bracket_left.step": "",
                "prints/shoulder/p2.3mf": "",
                "prints/shoulder/p2.gcode": "",
                "reviews/shoulder/20261106_p2-gate/brief.md": PIN,
                "manifests/layout-exceptions.txt": "",
                "scripts/macros/export.swp": "",
                "_wip/left-arm-layout/scratch_v2.SLDPRT": "",
                "_sim/shoulder/study.CWR": "",
                "_archive/checkpoints/old.SLDPRT": "",
                "shoulder/~$lock.SLDPRT": "",
            }
        )
        self.assertEqual(report.errors, [])
        self.assertEqual(report.warnings, [])

    def test_ignored_names(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            c = checker(Path(tmp))
            for name in [
                "x.step:Zone.Identifier",
                "~$a.SLDPRT",
                "Thumbs.db",
                ".cad-writer.lock",
            ]:
                self.assertTrue(c.ignored(name), name)
            self.assertFalse(c.ignored("marengo_shoulder_a.SLDPRT"))

    def test_forbidden_and_misplaced(self) -> None:
        report = run(
            [
                "codex/a.SLDPRT",
                "task-2/notes.txt",
                "parts/arm/x.SLDPRT",
                "exports/left_arm/render/UA.SLDPRT",
                "exports/loose.stl",
                "shoulder/fea.CWR",
                "shoulder/fea.STE",
                "stray.step",
            ]
        )
        joined = "\n".join(report.errors)
        for needle in [
            "codex: forbidden",
            "task-2: forbidden",
            "parts: unknown top-level",
            "UA.SLDPRT: native SolidWorks file in exports/",
            "loose.stl: exports/ must be split by region",
            "fea.CWR: simulation results",
            "fea.STE: simulation results",
            "stray.step: loose file",
        ]:
            self.assertIn(needle, joined)

    def test_region_file_types(self) -> None:
        report = run(
            [
                "shoulder/marengo_shoulder_bracket.step",
                "arm/marengo_arm_upper_link.SLDPRT",
                "prints/shoulder/marengo_shoulder_bracket.stl",
                "vendor/robstride/rs03/x.step.identifier",
            ]
        )
        joined = "\n".join(report.errors)
        self.assertIn("'.step' files do not belong in shoulder/", joined)
        self.assertIn(
            "arm/marengo_arm_upper_link.SLDPRT: '.sldprt' files do not belong in arm/ (whole-arm",
            joined,
        )
        self.assertIn("'.stl' files do not belong in prints/", joined)
        self.assertIn("'.identifier' files do not belong in vendor/", joined)

    def test_dated_folders(self) -> None:
        report = run(
            [
                "shoulder/LEFT_P2_20261105/a.SLDPRT",
                "reviews/shoulder/p2-gate-20261105/a.md",
            ]
        )
        joined = "\n".join(report.errors)
        self.assertIn("date-stamped", joined)
        self.assertIn("should be named <YYYYMMDD>_<topic>", joined)

    def test_review_needs_dated_packet_folder(self) -> None:
        report = run({"reviews/shoulder/brief.md": PIN, "reviews/notes.md": PIN})
        self.assertEqual(len(report.errors), 2)
        self.assertTrue(
            all(
                "<YYYYMMDD>_<topic>" in e or "split by region" in e
                for e in report.errors
            )
        )

    def test_review_limits_and_pin(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            pkt = "reviews/shoulder/20261108_gate"
            build(
                root,
                {
                    f"{pkt}/brief.md": "no pin here",
                    f"{pkt}/big.png": "",
                    f"{pkt}/huge.csv": "",
                },
            )
            for rel, size in [
                (f"{pkt}/big.png", 3_000_000),
                (f"{pkt}/huge.csv", 11_000_000),
            ]:
                with (root / rel).open("wb") as fh:
                    fh.truncate(size)
            report = checker(root).run()
        self.assertEqual(len(report.errors), 1)
        self.assertIn("huge.csv: 11.0 MB review file", report.errors[0])
        warns = "\n".join(report.warnings)
        self.assertIn("big.png: 3.0 MB", warns)
        self.assertIn("Pinned commit", warns)

    def test_checkpoints_are_warning(self) -> None:
        report = run(["shoulder/.checkpoints/marengo_shoulder_x.SLDPRT"])
        self.assertEqual(report.errors, [])
        self.assertTrue(any(".checkpoints" in w for w in report.warnings))

    def test_file_of_100mb_or_more_is_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            build(root, ["shoulder/marengo_shoulder_big.SLDPRT", "_wip/x/huge.step"])
            for rel in ["shoulder/marengo_shoulder_big.SLDPRT", "_wip/x/huge.step"]:
                with (root / rel).open("wb") as fh:
                    fh.truncate(gitlfs.MAX_FILE_BYTES)  # sparse file, no real disk use
            report = checker(root).run()
        self.assertEqual(len(report.errors), 1)
        self.assertIn("100 MB", report.errors[0])
        self.assertIn("shoulder/", report.errors[0])

    def test_lfs_pointer_uses_real_size(self) -> None:
        report = run({"shoulder/marengo_shoulder_big.SLDPRT": pointer(150_000_000)})
        self.assertEqual(len(report.errors), 1)
        self.assertIn("150.0 MB", report.errors[0])

    def test_version_suffix_copy_is_error(self) -> None:
        bad = [
            "upper-arm/marengo_upper_arm_left_revC.SLDPRT",
            "upper-arm/marengo_upper_arm_v2_left.SLDPRT",
            "exports/shoulder/marengo_shoulder_bracket_final.step",
            "prints/shoulder/bracket (2).3mf",
            "shoulder/Copy of marengo_shoulder_bracket.SLDPRT",
        ]
        report = run(
            bad
            + [
                "fixtures/marengo_fixtures_test_stand_rs03.SLDPRT",
                "upper-arm/marengo_upper_arm_reverse_stop.SLDPRT",
            ]
        )
        flagged = [e for e in report.errors if "version-suffixed" in e]
        self.assertEqual(sorted(e.split(":")[0] for e in flagged), sorted(bad))

    def test_bad_name_is_warning(self) -> None:
        report = run(["shoulder/P1_L01.SLDPRT"])
        self.assertEqual(report.errors, [])
        self.assertEqual(len(report.warnings), 1)

    def test_held_paths_do_not_fail(self) -> None:
        report = run(
            ["parts/arm/x.SLDPRT", "assemblies/a.SLDASM"], exceptions=["cad/parts/"]
        )
        self.assertEqual(len(report.held), 1)
        self.assertEqual(len(report.errors), 1)
        self.assertIn("assemblies", report.errors[0])


class ExportsManifestTests(unittest.TestCase):
    def test_stale_changed_and_unrecorded(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            cad = Path(tmp) / "cad"
            src, exp = (
                "shoulder/marengo_shoulder_b.SLDPRT",
                "exports/shoulder/marengo_shoulder_b.step",
            )
            build(
                cad,
                {
                    src: "v1",
                    exp: "e1",
                    "exports/shoulder/other.stl": "",
                    "manifests/exports.json": '{"exports": []}',
                },
            )
            manifest = cad / "manifests" / "exports.json"
            record_export.record(
                exp, f"cad/{src}", "Default", "AP214", cad_root=cad, manifest=manifest
            )
            data = json.loads(manifest.read_text())
            self.assertEqual(data["exports"][0]["source"], src)
            report = checker(cad, exports=data).run()
            self.assertEqual(report.errors, [])
            self.assertEqual(
                [w.split(":")[0] for w in report.warnings],
                ["exports/shoulder/other.stl"],
            )
            (cad / src).write_text("v2")
            (cad / exp).write_text("e2")
            report = checker(cad, exports=data).run()
            self.assertTrue(any("stale" in w for w in report.warnings))
            self.assertTrue(
                any("changed since it was recorded" in e for e in report.errors)
            )
            (cad / src).unlink()
            report = checker(cad, exports=data).run()
            self.assertTrue(any("is gone" in e for e in report.errors))


class LockTests(unittest.TestCase):
    def test_acquire_check_release_stale(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            lock = Path(tmp) / "cad" / ".cad-writer.lock"
            self.assertFalse(
                cad_lock.check("anyone", lock)[0]
            )  # committing CAD needs the lock
            self.assertTrue(cad_lock.acquire("left-arm", path=lock)[0])
            self.assertFalse(cad_lock.acquire("torso", path=lock)[0])
            self.assertTrue(cad_lock.check("left-arm", lock)[0])
            self.assertFalse(cad_lock.check("torso", lock)[0])
            self.assertFalse(cad_lock.check(None, lock)[0])
            self.assertFalse(cad_lock.release("torso", lock)[0])
            self.assertTrue(cad_lock.refresh("left-arm", lock)[0])
            self.assertEqual(json.loads(lock.read_text())["owner"], "left-arm")
            data = json.loads(lock.read_text())
            data["updated"] = time.time() - cad_lock.STALE_SECONDS - 1
            lock.write_text(json.dumps(data))
            self.assertFalse(cad_lock.check("torso", lock)[0])
            ok, msg = cad_lock.acquire("torso", path=lock)
            self.assertFalse(ok)  # stale takeover is explicit
            self.assertIn("--take-stale", msg)
            ok, msg = cad_lock.acquire("torso", path=lock, take_stale=True)
            self.assertTrue(ok)
            self.assertIn("stale", msg)
            self.assertTrue(cad_lock.release("torso", lock)[0])
            self.assertFalse(lock.exists())
            self.assertEqual(
                list(lock.parent.iterdir()), []
            )  # no temp files left behind

    def test_unreadable_lock_is_held_not_stale(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            lock = Path(tmp) / ".cad-writer.lock"
            lock.write_text("")  # e.g. a crash mid-write by some other tool
            self.assertFalse(cad_lock.acquire("torso", path=lock, take_stale=True)[0])
            self.assertFalse(cad_lock.check("torso", lock)[0])
            self.assertFalse(cad_lock.release("torso", lock)[0])
            self.assertTrue(lock.exists())


def git(root: Path, *args: str) -> str:
    env = dict(
        os.environ,
        GIT_AUTHOR_NAME="t",
        GIT_AUTHOR_EMAIL="t@t",
        GIT_COMMITTER_NAME="t",
        GIT_COMMITTER_EMAIL="t@t",
    )
    return subprocess.run(
        ["git", "-C", str(root), *args],
        check=True,
        capture_output=True,
        text=True,
        env=env,
    ).stdout


class GitTests(unittest.TestCase):
    """Size, budget, repo-mode and hook checks against a throwaway repo (pointers written by hand, no LFS needed)."""

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        git(self.root, "init", "-q", "-b", "main")
        git(self.root, "config", "commit.gpgsign", "false")
        build(
            self.root,
            {
                "README.md": "x",
                "cad/shoulder/marengo_shoulder_a.SLDPRT": pointer(5_000_000, "1" * 64),
            },
        )
        git(self.root, "add", "-A")
        git(self.root, "commit", "-qm", "base")
        git(self.root, "update-ref", "refs/remotes/origin/main", "HEAD")

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_staged_and_pushed_sizes_use_pointer(self) -> None:
        build(
            self.root,
            {
                "cad/shoulder/marengo_shoulder_big.SLDPRT": pointer(
                    120_000_000, "2" * 64
                )
            },
        )
        git(self.root, "add", "-A")
        staged = cfs.staged(self.root)
        self.assertEqual(
            staged, [(120_000_000, "cad/shoulder/marengo_shoulder_big.SLDPRT")]
        )
        git(self.root, "commit", "-qm", "big")
        pushed = cfs.pushed(["origin/main..HEAD"], self.root)
        self.assertEqual(
            pushed, [(120_000_000, "cad/shoulder/marengo_shoulder_big.SLDPRT")]
        )
        self.assertEqual(cfs.report(pushed, "pushed"), 1)

    def test_budget_counts_only_new_objects(self) -> None:
        budget = {
            "pushMaxBytes": 50_000_000,
            "monthMaxBytes": 10**12,
            "storageWarnBytes": 10**12,
            "storageMaxBytes": 10**12,
        }
        build(
            self.root,
            {
                "cad/shoulder/copy.SLDPRT": pointer(
                    5_000_000, "1" * 64
                ),  # already stored on the remote
                "cad/shoulder/marengo_shoulder_new.SLDPRT": pointer(
                    60_000_000, "3" * 64
                ),
            },
        )
        git(self.root, "add", "-A")
        git(self.root, "commit", "-qm", "new")
        ok, msgs = lfs_budget.check_push(["origin/main..HEAD"], self.root, budget)
        self.assertFalse(ok)
        self.assertIn("adds 60.0 MB in 1 new object", msgs[0])
        os.environ["CAD_BUDGET_OK"] = "1"
        try:
            self.assertTrue(
                lfs_budget.check_push(["origin/main..HEAD"], self.root, budget)[0]
            )
        finally:
            del os.environ["CAD_BUDGET_OK"]

    def test_pre_push_rev_args(self) -> None:
        z = gitlfs.ZERO
        text = f"refs/heads/a {'1' * 40} refs/heads/a {'2' * 40}\nrefs/heads/b {'3' * 40} refs/heads/b {z}\nrefs/heads/c {z} refs/heads/c {'4' * 40}\n"
        self.assertEqual(
            gitlfs.pre_push_rev_args(text, "origin"),
            ["1" * 40, "3" * 40, f"^{'2' * 40}", "--not", "--remotes=origin"],
        )
        self.assertEqual(
            gitlfs.pre_push_rev_args(
                f"refs/heads/c {z} refs/heads/c {'4' * 40}\n", "origin"
            ),
            [],
        )

    def test_merge_content_is_checked(self) -> None:
        git(self.root, "checkout", "-q", "-b", "side")
        build(self.root, {"side.txt": "s"})
        git(self.root, "add", "-A")
        git(self.root, "commit", "-qm", "side")
        git(self.root, "checkout", "-q", "main")
        build(self.root, {"main.txt": "m"})
        git(self.root, "add", "-A")
        git(self.root, "commit", "-qm", "main")
        git(self.root, "merge", "-q", "--no-commit", "side")
        build(
            self.root,
            {"cad/shoulder/marengo_shoulder_m.SLDPRT": pointer(130_000_000, "4" * 64)},
        )
        git(self.root, "add", "-A")
        git(self.root, "commit", "-qm", "merge with new content")
        sizes = cfs.pushed(["HEAD", "--not", "--remotes=origin"], self.root)
        self.assertIn((130_000_000, "cad/shoulder/marengo_shoulder_m.SLDPRT"), sizes)
        budget = {
            "pushMaxBytes": 10**12,
            "monthMaxBytes": 10**12,
            "storageWarnBytes": 10**12,
            "storageMaxBytes": 10**12,
        }
        _, msgs = lfs_budget.check_push(
            ["HEAD", "--not", "--remotes=origin"], self.root, budget
        )
        self.assertIn("adds 130.0 MB in 1 new object", msgs[0])

    def test_repo_mode_reads_index_and_needs_lfs(self) -> None:
        exp, src = (
            "exports/shoulder/marengo_shoulder_a.step",
            "shoulder/marengo_shoulder_a.SLDPRT",
        )
        build(
            self.root,
            {
                f"cad/{exp}": pointer(1000, "5" * 64),
                "cad/manifests/exports.json": json.dumps(
                    {
                        "exports": [
                            {
                                "export": exp,
                                "source": src,
                                "config": "",
                                "settings": "",
                                "sourceSha256": "1" * 64,
                                "exportSha256": "5" * 64,
                                "recorded": "",
                            }
                        ]
                    }
                ),
                "cad/shoulder/marengo_shoulder_raw.SLDPRT": "not a pointer",
                "cad/shoulder/desktop.ini:Zone.Identifier".replace(":", "_"): "",
                "cad/.cad-writer.lock": "{}",
            },
        )
        git(self.root, "add", "-A", "-f")
        (self.root / "cad" / exp).write_text("edited in the tree, not staged")
        report = ccl.check_repo(
            ccl.load_json(ccl.LAYOUT_PATH),
            ccl.load_json(ccl.CONVENTIONS_PATH),
            self.root,
        )
        joined = "\n".join(report.errors)
        self.assertNotIn(
            "changed since it was recorded", joined
        )  # the staged export matches
        self.assertIn(
            "marengo_shoulder_raw.SLDPRT: CAD file committed as a plain git blob",
            joined,
        )
        self.assertIn(
            "cad/.cad-writer.lock: lock or download-marker file must not be tracked",
            joined,
        )
        self.assertIn("Zone.Identifier: lock or download-marker", joined)

    def test_repo_mode(self) -> None:
        build(
            self.root,
            {
                "hardware/designs/x/part.step": "",
                "hardware/designs/held/part.stl": "",
                "hardware/designs/x/_wip/notes.md": "",
                "cad/manifests/layout-exceptions.txt": "# held\nhardware/designs/held/\n",
            },
        )
        git(self.root, "add", "-A", "-f")
        report = ccl.check_repo(
            ccl.load_json(ccl.LAYOUT_PATH),
            ccl.load_json(ccl.CONVENTIONS_PATH),
            self.root,
        )
        joined = "\n".join(report.errors)
        self.assertIn("hardware/designs/x/part.step: CAD file outside cad/", joined)
        self.assertIn(
            "hardware/designs/x/_wip/notes.md: tracked file in a scratch folder", joined
        )
        self.assertEqual(len(report.errors), 2)
        self.assertEqual(len(report.held), 1)

    def test_install_hooks_keeps_existing(self) -> None:
        hooks = self.root / ".git" / "hooks"
        hooks.mkdir(exist_ok=True)
        (hooks / "pre-push").write_text('#!/bin/sh\ngit lfs pre-push "$@"\n')
        (hooks / "pre-commit").write_text("#!/bin/sh\necho mine\n")
        self.assertFalse(install_hooks.is_current(self.root))
        lines = "\n".join(install_hooks.install(self.root))
        self.assertIn(
            "replaced Git LFS's pre-push hook", lines
        )  # ours runs git lfs pre-push itself
        self.assertIn("kept the existing pre-commit hook as pre-commit.local", lines)
        self.assertFalse((hooks / "pre-push.local").exists())
        self.assertTrue(install_hooks.is_current(self.root))
        install_hooks.install(self.root)  # idempotent: does not wrap itself
        self.assertIn("echo mine", (hooks / "pre-commit.local").read_text())
        self.assertIn("git lfs pre-push", (hooks / "pre-push").read_text())


class SwRefsTests(unittest.TestCase):
    def test_classify(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            cad = Path(tmp) / "cad"
            build(
                Path(tmp),
                ["cad/shoulder/a.SLDPRT", "cad/_wip/x/b.SLDPRT", "outside/c.SLDPRT"],
            )
            self.assertIsNone(
                check_sw_refs.classify(str(cad / "shoulder/a.SLDPRT"), cad)
            )
            self.assertIn(
                "scratch", check_sw_refs.classify(str(cad / "_wip/x/b.SLDPRT"), cad)
            )
            self.assertIn(
                "outside cad/",
                check_sw_refs.classify(str(Path(tmp) / "outside/c.SLDPRT"), cad),
            )
            self.assertIn(
                "missing", check_sw_refs.classify(str(cad / "gone.SLDPRT"), cad)
            )
            self.assertIn(
                "gitignored",
                check_sw_refs.classify(
                    str(cad / "shoulder/a.SLDPRT"), cad, {"shoulder/a.sldprt"}
                ),
            )
            self.assertIn(
                "not in git yet",
                check_sw_refs.classify(
                    str(cad / "shoulder/a.SLDPRT"), cad, set(), set()
                ),
            )
            self.assertIsNone(
                check_sw_refs.classify(
                    str(cad / "shoulder/a.SLDPRT"), cad, set(), {"shoulder/a.sldprt"}
                )
            )


if __name__ == "__main__":
    unittest.main()
