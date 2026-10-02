"""Check a completed passive observation; exit 42 means CAN error reproduced.

A clean bounded window is an observation, not physical recovery or motion proof.
"""
from pathlib import Path
import collections
import hashlib
import json
import re
import sys

stage = Path(sys.argv[1]).resolve(strict=True)
receipt = json.loads((stage / "diagnostic-receipt.json").read_text())
raw = (stage / "passive-can.log").read_bytes()
assert len(raw) <= 24 * 1024 * 1024
assert hashlib.sha256(raw).hexdigest() == receipt["capture_SHA256"]
assert receipt["restart_exit"] == 0 and receipt["passive_capture_exit"] in (0, 124)
assert receipt["policy_calibration_and_runtime_environment_unchanged"]
assert receipt["capture_elapsed_seconds"] >= 55
assert len(receipt["snapshot_samples"]) >= 50
assert all(sample.get("disabled") for sample in receipt["snapshot_samples"])

frames = []
for line in raw.decode().splitlines():
    match = re.fullmatch(r"\((\d+\.\d+)\)\s+(can[01])\s+([0-9A-Fa-f]+)#([0-9A-Fa-f]*)", line)
    assert match, line
    frames.append((float(match[1]), match[2], int(match[3], 16), match[4], line))
assert len(frames) == receipt["frame_count"]
duration = frames[-1][0] - frames[0][0]
assert duration >= 55
errors = [frame for frame in frames if frame[2] & 0x20000000]
requests = []
periodic = collections.Counter()
for frame in frames:
    _, interface, can_id, data, _ = frame
    if interface != "can0" or can_id & 0x20000000:
        continue
    kind, source, destination = can_id >> 24, (can_id >> 8) & 255, can_id & 255
    assert not (source == 253 and 1 <= destination <= 5 and kind in (3, 6)), "Enable/SetZero is outside the observation"
    if kind == 24 and source == 253 and 1 <= destination <= 5 and data == "0102030405060100":
        requests.append(frame)
    if kind == 24 and destination == 253 and 1 <= source <= 5:
        periodic[source] += 1
assert set(periodic) == set(range(1, 6))
assert all(90 <= count / duration <= 110 for count in periodic.values()), periodic
assert 4 <= len(requests) / duration <= 6, "Reporting traffic must remain comparable"
gaps = sorted((right[0] - left[0]) * 1000 for left, right in zip(requests, requests[1:]))
result = {
    "verdict": "CAN_ERROR_REPRODUCED" if errors else "NO_CAN_ERROR_IN_BOUNDED_WINDOW",
    "capture_SHA256": receipt["capture_SHA256"],
    "captured_seconds": duration,
    "frames": len(frames),
    "error_frames": [frame[4] for frame in errors],
    "rx_over_errors_before_after": [receipt[key]["can0"]["rx_over_errors"] for key in ("before_statistics", "after_statistics")],
    "periodic_feedback_frames_by_motor": dict(periodic),
    "reporting_requests": len(requests),
    "request_gap_ms_min_median_max": [gaps[0], gaps[len(gaps) // 2], gaps[-1]],
    "limits": "Software delivery timestamps; bounded observation only. No physical recovery, reference, stop or motion acceptance.",
}
print(json.dumps(result, indent=2))
sys.exit(42 if errors else 0)
