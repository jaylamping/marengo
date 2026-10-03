#!/usr/bin/env python3
"""Print the latest physical reference journal rows without opening CAN.

Reference grants live only inside the process that acquired them (ADR 0036);
the journal Davout writes beside the calibration record is history and never
grants a reference. This opens the SQLite file read-only (`mode=ro`), decodes
each row's `MRREF` body (crates/davout/src/reference_codec.rs) and prints one
summary line per row, newest first.

Journal path, mirroring `marengo_config::resolve_reference_journal_path`:
explicit argument, else `MARENGO_REFERENCE_JOURNAL`, else
`reference-journal.sqlite3` beside `MARENGO_CALIBRATION_RECORD`, else beside
`homing.yaml` `calibration_record_path` under `MARENGO_ROOT`.

Exit 0 when the journal was read or does not exist; 1 when it is unreadable.
"""

from __future__ import annotations

import argparse
import os
import re
import sqlite3
import struct
import sys
from pathlib import Path
from typing import Any
from urllib.parse import quote

JOURNAL_FILE = "reference-journal.sqlite3"
MAGIC = b"MRREF\0\x01\0"
DEPTH_CAPACITY = 64

# Fixed-width numeric tags of reference_codec::write_value (little-endian).
_NUMBERS = {
    2: "<B",
    3: "<H",
    4: "<I",
    5: "<Q",
    6: "<b",
    7: "<h",
    8: "<i",
    9: "<q",
    11: "<f",
    12: "<d",
}


class DecodeError(ValueError):
    pass


class _Reader:
    def __init__(self, data: bytes) -> None:
        self.data = data
        self.cursor = 0

    def take(self, count: int) -> bytes:
        end = self.cursor + count
        if end > len(self.data):
            raise DecodeError("truncated field")
        chunk = self.data[self.cursor : end]
        self.cursor = end
        return chunk

    def length(self) -> int:
        return struct.unpack("<I", self.take(4))[0]

    def value(self, depth: int = 0) -> Any:
        if depth > DEPTH_CAPACITY:
            raise DecodeError("decode depth")
        tag = self.take(1)[0]
        if tag in (0, 1):
            return tag == 1
        if tag in _NUMBERS:
            fmt = _NUMBERS[tag]
            return struct.unpack(fmt, self.take(struct.calcsize(fmt)))[0]
        if tag in (10, 13):
            return int.from_bytes(self.take(16), "little", signed=tag == 13)
        if tag == 14:
            try:
                return self.take(self.length()).decode("utf-8")
            except UnicodeDecodeError as exc:
                raise DecodeError("invalid UTF-8") from exc
        if tag == 15:
            return self.take(self.length())
        if tag == 16:
            return [self.value(depth + 1) for _ in range(self.length())]
        if tag == 17:
            fields: dict[str, Any] = {}
            for _ in range(self.length()):
                key = self.value(depth + 1)
                if not isinstance(key, str):
                    raise DecodeError("nonstring field key")
                fields[key] = self.value(depth + 1)
            return fields
        if tag == 18 or tag == 20:
            return None
        if tag == 19:
            return self.value(depth + 1)
        raise DecodeError(f"unsupported tag {tag}")


def decode_body(body: bytes) -> dict[str, Any]:
    """Decode one `reference_events.body` into nested dicts/lists/scalars."""
    if not body.startswith(MAGIC):
        raise DecodeError("missing MRREF magic")
    reader = _Reader(body)
    reader.cursor = len(MAGIC)
    value = reader.value()
    if reader.cursor != len(body):
        raise DecodeError("trailing bytes")
    if not isinstance(value, dict):
        raise DecodeError("body is not a struct")
    return value


def summarize(session: int, job: int, body: bytes) -> str:
    head = f"session={session} job={job}"
    try:
        event = decode_body(body)
        capture = event["capture"]
        address = capture["address"]
        audit = capture["audit"]
        physical = capture.get("physical")
        parts = [
            head,
            str(event["evidence_class"]),
            str(capture["joint"]),
            f"{address['interface']}/{address['device_id']}",
            f"pos={capture['position_rad']:+.4f} rad",
            f"confirmed={str(capture['confirmed']).lower()}",
            f"sign_verified={str(capture['sign_verified']).lower()}",
        ]
        if isinstance(physical, dict):
            parts.append(f"uid=0x{physical['device_uid']:016x}")
        parts.append(f"operator={audit['operator']}")
        parts.append(f"owner={audit['session']}")
        return " ".join(parts)
    except (DecodeError, KeyError, TypeError, ValueError) as exc:
        return f"{head} (undecodable body: {exc})"


def _calibration_record(root: Path, config_dir: Path) -> Path:
    record = os.environ.get("MARENGO_CALIBRATION_RECORD")
    if record:
        return Path(record)
    homing = config_dir / "homing.yaml"
    try:
        text = homing.read_text(encoding="utf-8")
    except OSError as exc:
        raise FileNotFoundError(f"cannot read {homing}: {exc.strerror}") from exc
    match = re.search(r"^\s*calibration_record_path:\s*(\S[^#\n]*)", text, re.MULTILINE)
    if not match:
        raise FileNotFoundError(f"no calibration_record_path in {homing}")
    return root / match.group(1).strip().strip("\"'")


def resolve_journal(explicit: str | None) -> Path:
    if explicit:
        return Path(explicit)
    env = os.environ.get("MARENGO_REFERENCE_JOURNAL")
    if env:
        return Path(env)
    root = Path(os.environ.get("MARENGO_ROOT") or Path(__file__).resolve().parent.parent)
    config_env = os.environ.get("MARENGO_CONFIG_DIR")
    if config_env:
        config_dir = Path(config_env)
        if not config_dir.is_absolute():
            config_dir = root / config_dir
    elif Path("/opt/marengo/config").is_dir():
        config_dir = Path("/opt/marengo/config")
    else:
        config_dir = root / "config"
    return _calibration_record(root, config_dir).parent / JOURNAL_FILE


def latest_rows(path: Path, limit: int) -> list[tuple[int, int, bytes]]:
    uri = f"file:{quote(str(path.resolve()))}?mode=ro"
    connection = sqlite3.connect(uri, uri=True, timeout=0)
    try:
        rows = connection.execute(
            "SELECT session, job, body FROM reference_events "
            "ORDER BY session DESC, job DESC LIMIT ?",
            (limit,),
        ).fetchall()
    finally:
        connection.close()
    return [(int(session), int.from_bytes(job, "big"), bytes(body)) for session, job, body in rows]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("journal", nargs="?", help="journal path (default: resolved like Davout)")
    parser.add_argument("--limit", type=int, default=5, help="rows to print (default 5)")
    args = parser.parse_args(argv)
    if args.limit < 1:
        parser.error("--limit must be at least 1")

    try:
        path = resolve_journal(args.journal)
    except FileNotFoundError as exc:
        print(f"reference journal: path unresolved ({exc})")
        return 1
    if not path.exists():
        print(f"reference journal: none at {path}")
        return 0
    try:
        rows = latest_rows(path, args.limit)
    except sqlite3.Error as exc:
        print(f"reference journal: unreadable at {path}: {exc}")
        return 1

    print(f"reference journal (history only; never grants a reference): {path}")
    if not rows:
        print("  (no rows)")
    for session, job, body in rows:
        print(f"  {summarize(session, job, body)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
