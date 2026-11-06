"""Tests for check_cad_layout.py (stdlib unittest; run by scripts/check.sh)."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_cad_layout as ccl  # noqa: E402


def build(root: Path, files: list[str]) -> None:
    for rel in files:
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.touch()


def run(files: list[str]) -> ccl.Report:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        build(root, files)
        layout = ccl.load_json(ccl.LAYOUT_PATH)
        conventions = ccl.load_json(ccl.CONVENTIONS_PATH)
        return ccl.Checker(root, layout, conventions).run()


class LayoutTests(unittest.TestCase):
    def test_clean_tree(self) -> None:
        report = run([
            "README.md",
            "marengo.SLDASM",
            "shoulder/marengo_shoulder_pitch_bracket_left.SLDPRT",
            "arm/marengo_arm_left_asm.SLDASM",
            "vendor/robstride/rs03/vendor_robstride_rs03_vendor.SLDPRT",
            "exports/shoulder/marengo_shoulder_pitch_bracket_left.step",
            "prints/shoulder/p2.3mf",
            "reviews/shoulder/20261106_p2-gate/brief.md",
            "_wip/left-arm-layout/scratch.SLDPRT",
            "_sim/shoulder/study.CWR",
            "_archive/checkpoints/old.SLDPRT",
            "shoulder/~$lock.SLDPRT",
        ])
        self.assertEqual(report.errors, [])
        self.assertEqual(report.warnings, [])

    def test_forbidden_and_misplaced(self) -> None:
        report = run([
            "codex/a.SLDPRT",
            "task-2/notes.txt",
            "parts/arm/.checkpoints/x.SLDPRT",
            "exports/left_arm/render/UA.SLDPRT",
            "exports/loose.stl",
            "shoulder/fea.CWR",
            "stray.step",
        ])
        joined = "\n".join(report.errors)
        for needle in [
            "codex: forbidden",
            "task-2: forbidden",
            "parts: unknown top-level",
            ".checkpoints: forbidden",
            "UA.SLDPRT: native SolidWorks file in exports/",
            "loose.stl: exports/ must be split by region",
            "fea.CWR: simulation results",
            "stray.step: loose file",
        ]:
            self.assertIn(needle, joined)

    def test_dated_folder_outside_reviews(self) -> None:
        report = run(["shoulder/LEFT_P2_20261105/a.SLDPRT"])
        self.assertTrue(any("date-stamped" in e for e in report.errors))

    def test_bad_name_is_warning(self) -> None:
        report = run(["shoulder/P1_L01.SLDPRT"])
        self.assertEqual(report.errors, [])
        self.assertEqual(len(report.warnings), 1)


if __name__ == "__main__":
    unittest.main()
