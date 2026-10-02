"""Summarize saved receipts and passive captures; never connects to the robot."""

import collections
import json
import math
from pathlib import Path
import re


def summarize(folder):
    audit = json.loads((folder / "reference-audit.json").read_text())
    frames = []
    for line in (folder / "candump.log").read_text().splitlines():
        match = re.match(r"\(([\d.]+)\)\s+can0\s+([A-Fa-f0-9]+)#([A-Fa-f0-9]+)", line)
        if match:
            timestamp, arbitration, payload = match.groups()
            frames.append((float(timestamp), int(arbitration, 16), bytes.fromhex(payload)))
    enables = [f for f in frames if (f[1] >> 24) & 31 == 3]
    stops = [f for f in frames if (f[1] >> 24) & 31 == 4]
    mit = [f for f in frames if (f[1] >> 24) & 31 == 1]
    statuses = [
        f for f in frames
        if (f[1] >> 24) & 31 == 2 and f[2][:3] != bytes.fromhex("00c456")
    ]
    neutral = bytes.fromhex("7fff7fff00000000")
    result = {
        "folder": folder.name,
        "source": (folder / "source-revision.txt").read_text().strip(),
        "started_utc": (folder / "start-time.txt").read_text().strip(),
        "exit_status": int((folder / "exit-status.txt").read_text()),
        "error": (folder / "lower-yaw.stderr").read_text().strip(),
        "profile": audit["lower_yaw_profile"],
        "hashes_unchanged": (folder / "sha256-before.txt").read_bytes()
        == (folder / "sha256-after.txt").read_bytes(),
        "enable_counts_by_id": dict(collections.Counter(f[1] & 255 for f in enables)),
        "gained_mit_counts_by_id": dict(collections.Counter(
            f[1] & 255 for f in mit if f[2] != neutral
        )),
        "neighbors_neutral": all(f[2] == neutral for f in mit if f[1] & 255 != 5),
        "all_feedforward_zero": all((f[1] >> 8) & 65535 == 32767 for f in mit),
        "status_fault_flags": dict(collections.Counter((f[1] >> 16) & 63 for f in statuses)),
        "status_drive_modes": dict(collections.Counter((f[1] >> 22) & 3 for f in statuses)),
        "last_disable_after_first_enable_ms": (
            (max(f[0] for f in stops) - min(f[0] for f in enables)) * 1000
            if enables else None
        ),
        "runtime_state": (folder / "runtime-state.txt").read_text().strip(),
    }
    assert result["hashes_unchanged"] and result["neighbors_neutral"]
    assert result["all_feedforward_zero"]
    assert set(result["gained_mit_counts_by_id"]) <= {5}
    receipt_path = folder / "lower-yaw.json"
    if receipt_path.stat().st_size:
        receipt = json.loads(receipt_path.read_text())
        samples = receipt["samples"]
        yaw = [next(f for f in s["feedback"] if f["joint"] == receipt["joint"])
               for s in samples]
        positions = [f["position_rad"] for f in yaw]
        result["tracking"] = {
            "samples": len(samples),
            "peak_position_rad": max(positions),
            "final_position_rad": positions[-1],
            "rms_error_rad": math.sqrt(sum(
                (s["target_rad"] - f["position_rad"]) ** 2
                for s, f in zip(samples, yaw)
            ) / len(samples)),
            "max_canonical_velocity_rad_s": max(abs(f["velocity_rad_s"]) for f in yaw),
            "max_abs_reported_torque_nm": max(abs(f["torque_nm"]) for f in yaw),
            "elapsed_us": receipt["elapsed_us"],
            "max_tick_delay_us": receipt["max_tick_delay_us"],
            "stop_writes": receipt["stop_writes"],
            "failed_stop_writes": receipt["failed_stop_writes"],
        }
    return result


if __name__ == "__main__":
    attempts = [summarize(folder) for folder in Path(__file__).parent.glob("yaw-*")
                if folder.is_dir()]
    print(json.dumps(sorted(attempts, key=lambda r: r["started_utc"]), indent=2))
