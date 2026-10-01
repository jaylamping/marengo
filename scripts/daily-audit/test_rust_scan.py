"""Independent production/test lexical boundary and audit evidence controls."""
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import audit
from rust_scan import production_view


class RustScanTests(unittest.TestCase):
    def check(self, source):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = "crates/berthier/src/fixture.rs"
            file = root / path
            file.parent.mkdir(parents=True)
            file.write_text(source)
            report = audit.Report(date="2026-10-01")
            with patch.object(audit, "ROOT", root):
                audit.check_davout_bypass([path], report)
            return report

    def test_comments_literals_and_test_braces_do_not_hide_following_production(self):
        source = '''// robstride::send()
/* nested /* robstride::send() */ comment */
const MESSAGE: &str = r###"} robstride::send() {"###;
#[cfg(test)]
mod tests {
    fn fixture() { let _ = "}"; robstride::send(); }
}
fn live() { socketcan::CanSocket::open("can0"); }
'''
        report = self.check(source)
        self.assertEqual(len(report.findings), 1)
        self.assertEqual(report.findings[0].severity, "critical")
        self.assertIn("line 8", report.findings[0].message)

    def test_test_function_and_integration_harness_are_excluded(self):
        report = self.check('#[cfg(test)]\nfn harness() { robstride::send(); }\nfn live() {}')
        self.assertTrue(report.clean)

    def test_real_driver_call_is_critical(self):
        report = self.check('fn live() { robstride::send(); }')
        self.assertFalse(report.clean)
        self.assertEqual(report.findings[0].severity, "critical")
        self.assertIn("line 1", report.findings[0].message)

    def test_compound_cfg_is_unknown_not_success(self):
        report = self.check('#[cfg(any(test, feature="live"))]\nfn gated() { robstride::send(); }')
        self.assertFalse(report.clean)
        self.assertEqual(report.findings[0].category, "scan")
        self.assertIn("Unknown", report.findings[0].message)

    def test_unterminated_source_is_unknown(self):
        report = self.check('/* unfinished')
        self.assertFalse(report.clean)
        self.assertEqual(report.findings[0].category, "scan")

    def test_lifetimes_and_character_braces_preserve_lines(self):
        source = "fn borrow<'a>(x: &'a str) { let c = '}'; }\nrobstride::send();"
        view = production_view(source)
        self.assertEqual(len(view), len(source))
        self.assertEqual(view.count('\n'), source.count('\n'))
        self.assertIn("'a", view)
        self.assertNotIn("'}'", view)


if __name__ == "__main__":
    unittest.main()
