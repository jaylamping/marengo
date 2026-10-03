#!/usr/bin/env python3
"""Offline tests for reference-journal-tail.py (no Pi, no CAN)."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import os
import sqlite3
import struct
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).resolve().parent / "reference-journal-tail.py"
_SPEC = importlib.util.spec_from_file_location("reference_journal_tail", SCRIPT)
assert _SPEC and _SPEC.loader
tail = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(tail)


def _string(text: str) -> bytes:
    raw = text.encode()
    return b"\x0e" + struct.pack("<I", len(raw)) + raw


def _map(fields: dict[str, bytes]) -> bytes:
    """Struct as reference_codec writes it: tag 17, count, sorted string keys."""
    out = b"\x11" + struct.pack("<I", len(fields))
    for key in sorted(fields):
        out += _string(key) + fields[key]
    return out


def _u8(n: int) -> bytes:
    return b"\x02" + struct.pack("<B", n)


def _u64(n: int) -> bytes:
    return b"\x05" + struct.pack("<Q", n)


def _f32(x: float) -> bytes:
    return b"\x0b" + struct.pack("<f", x)


def _body(joint: str, device_id: int, pos: float, uid: int | None) -> bytes:
    physical = b"\x12" if uid is None else b"\x13" + _map({"device_uid": _u64(uid), "ack_can_id": b"\x04" + struct.pack("<I", 2)})
    capture = _map(
        {
            "joint": _string(joint),
            "address": _map({"interface": _string("can0"), "device_id": _u8(device_id)}),
            "position_rad": _f32(pos),
            "confirmed": b"\x01",
            "sign_verified": b"\x01",
            "audit": _map({"operator": _string("bench"), "session": _string("marengo-pi-42")}),
            "physical": physical,
            "reporting": b"\x10" + struct.pack("<I", 1) + b"\x14",
        }
    )
    evidence = "closed_virtual" if uid is None else "physical_robstride"
    return tail.MAGIC + _map({"evidence_class": _string(evidence), "capture": capture, "version": b"\x04" + struct.pack("<I", 1)})


def _journal(path: Path, rows: list[tuple[int, int, bytes]]) -> None:
    connection = sqlite3.connect(path)
    connection.execute(
        "CREATE TABLE reference_events(session INTEGER NOT NULL, job BLOB NOT NULL, "
        "body BLOB NOT NULL, checksum BLOB NOT NULL, PRIMARY KEY(session,job)) WITHOUT ROWID"
    )
    connection.executemany(
        "INSERT INTO reference_events VALUES (?, ?, ?, ?)",
        [(session, job.to_bytes(8, "big"), body, bytes(32)) for session, job, body in rows],
    )
    connection.commit()
    connection.close()


def _run(*argv: str, env: dict[str, str] | None = None) -> tuple[int, str]:
    out = io.StringIO()
    with mock.patch.dict(os.environ, env or {}, clear=False), contextlib.redirect_stdout(out):
        code = tail.main(list(argv))
    return code, out.getvalue()


class ReferenceJournalTailTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_prints_latest_rows_newest_first(self) -> None:
        journal = self.root / "journal.sqlite3"
        _journal(
            journal,
            [
                (1, 1, _body("right_shoulder_pitch", 1, 0.5, 0xAB)),
                (2, 1, _body("right_shoulder_pitch", 1, 0.0012, 0x0102030405060708)),
                (2, 2, _body("right_elbow_pitch", 4, -0.0008, None)),
            ],
        )
        code, out = _run(str(journal), "--limit", "2")
        self.assertEqual(code, 0)
        lines = out.splitlines()
        self.assertEqual(lines[0], f"reference journal (history only; never grants a reference): {journal}")
        self.assertEqual(
            lines[1],
            "  session=2 job=2 closed_virtual right_elbow_pitch can0/4 pos=-0.0008 rad "
            "confirmed=true sign_verified=true operator=bench owner=marengo-pi-42",
        )
        self.assertEqual(
            lines[2],
            "  session=2 job=1 physical_robstride right_shoulder_pitch can0/1 pos=+0.0012 rad "
            "confirmed=true sign_verified=true uid=0x0102030405060708 operator=bench owner=marengo-pi-42",
        )
        self.assertEqual(len(lines), 3)

    def test_opens_read_only(self) -> None:
        journal = self.root / "journal.sqlite3"
        _journal(journal, [(1, 1, _body("j", 1, 0.0, None))])
        before = journal.read_bytes()
        journal.chmod(0o444)
        code, _ = _run(str(journal))
        self.assertEqual(code, 0)
        self.assertEqual(journal.read_bytes(), before)

    def test_undecodable_body_is_reported_per_row(self) -> None:
        journal = self.root / "journal.sqlite3"
        _journal(journal, [(3, 7, b"not a body")])
        code, out = _run(str(journal))
        self.assertEqual(code, 0)
        self.assertIn("session=3 job=7 (undecodable body: missing MRREF magic)", out)

    def test_missing_journal_is_not_created(self) -> None:
        journal = self.root / "absent.sqlite3"
        code, out = _run(str(journal))
        self.assertEqual(code, 0)
        self.assertEqual(out.strip(), f"reference journal: none at {journal}")
        self.assertFalse(journal.exists())

    def test_resolves_beside_homing_calibration_record(self) -> None:
        config = self.root / "config"
        config.mkdir()
        (config / "homing.yaml").write_text(
            "homing:\n  calibration_record_path: \"var/calibration/zero_registry.yaml\"  # history\n"
        )
        env = {"MARENGO_ROOT": str(self.root), "MARENGO_CONFIG_DIR": str(config)}
        with mock.patch.dict(os.environ, env):
            os.environ.pop("MARENGO_REFERENCE_JOURNAL", None)
            os.environ.pop("MARENGO_CALIBRATION_RECORD", None)
            resolved = tail.resolve_journal(None)
        self.assertEqual(resolved, self.root / "var/calibration/reference-journal.sqlite3")

    def test_env_journal_overrides_calibration_record(self) -> None:
        env = {"MARENGO_REFERENCE_JOURNAL": "/x/j.sqlite3", "MARENGO_CALIBRATION_RECORD": "/y/r.yaml"}
        with mock.patch.dict(os.environ, env):
            self.assertEqual(tail.resolve_journal(None), Path("/x/j.sqlite3"))
            os.environ.pop("MARENGO_REFERENCE_JOURNAL")
            self.assertEqual(tail.resolve_journal(None), Path("/y/reference-journal.sqlite3"))


if __name__ == "__main__":
    unittest.main()
