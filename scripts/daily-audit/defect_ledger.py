"""Durable audit observations; only explicit bound repair evidence can resolve one."""
import hashlib
import json
import os
import re
from pathlib import Path
import tempfile


def _valid_receipt(row: dict, receipt: dict) -> bool:
    return (receipt.get("finding_id") == row["id"] and receipt.get("result") == "passed"
            and receipt.get("observed_evidence_sha256") == row["observed_evidence_sha256"]
            and isinstance(receipt.get("repair_commit"), str)
            and re.fullmatch(r"[0-9a-f]{40}", receipt["repair_commit"]) is not None
            and isinstance(receipt.get("validation_command"), str)
            and bool(receipt["validation_command"].strip()))


def _load(path: Path) -> dict:
    if not path.exists():
        return {"schema_version": 1, "findings": {}}
    payload = json.loads(path.read_text())
    if not isinstance(payload, dict):
        raise ValueError("invalid ledger document")
    if payload.get("schema_version") != 1 or not isinstance(payload.get("findings"), dict):
        raise ValueError("unknown defect ledger schema")
    for key, row in payload["findings"].items():
        if not isinstance(row, dict) or row.get("id") != key or row.get("state") not in {"open", "resolved"}:
            raise ValueError("invalid defect ledger entry")
        if row["state"] == "resolved":
            resolution = row.get("resolution", {})
            raw = resolution.get("evidence_json", "").encode()
            if (hashlib.sha256(raw).hexdigest() != resolution.get("evidence_sha256")
                    or not _valid_receipt(row, json.loads(raw))):
                raise ValueError("resolved ledger entry lacks bound repair evidence")
    return payload


def _write(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, name = tempfile.mkstemp(prefix=".defect-ledger-", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w") as stream:
            json.dump(payload, stream, indent=2)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(name, path)
    finally:
        if os.path.exists(name):
            os.unlink(name)


def _locked(path: Path):
    path.parent.mkdir(parents=True, exist_ok=True)
    return os.open(str(path) + ".lock", os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)


def record_findings(path: Path, findings: list[dict], observed_at: str) -> list[dict]:
    descriptor = _locked(path)
    try:
        payload = _load(path)
        for finding in findings:
            if finding["severity"] not in {"warn", "critical"}:
                continue
            encoded = json.dumps(finding, sort_keys=True, separators=(",", ":")).encode()
            identity = hashlib.sha256(encoded).hexdigest()
            row = payload["findings"].get(identity)
            if row is None:
                row = {"id": identity, "state": "open", "first_seen": observed_at,
                       "finding": finding, "observed_evidence_sha256": identity}
                payload["findings"][identity] = row
            # Recurrence explicitly reopens an accepted resolution; absence never closes it.
            row.update(last_seen=observed_at, state="open")
        _write(path, payload)
        return [row for row in payload["findings"].values() if row["state"] == "open"]
    finally:
        os.close(descriptor)
        Path(str(path) + ".lock").unlink()


def accept_resolution(path: Path, finding_id: str, evidence: Path, expected_sha256: str) -> None:
    """Explicit caller acceptance; never called merely because a scan is empty."""
    raw = evidence.read_bytes()
    if hashlib.sha256(raw).hexdigest() != expected_sha256:
        raise ValueError("repair evidence hash mismatch")
    receipt = json.loads(raw)
    descriptor = _locked(path)
    try:
        payload = _load(path)
        row = payload["findings"][finding_id]
        if not _valid_receipt(row, receipt):
            raise ValueError("repair evidence does not bind this finding and successful validation")
        row.update(state="resolved", resolution={"evidence_sha256": expected_sha256,
            "evidence_json": raw.decode(), "receipt": receipt})
        _write(path, payload)
    finally:
        os.close(descriptor)
        Path(str(path) + ".lock").unlink()
