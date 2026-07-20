#!/usr/bin/env python3
"""Keep marengo-pi MCP enabled in Cursor's workspace storage.

Cursor stores per-workspace disabled MCP identifiers in
`cursor/disabledMcpServers`. It also hashes `.cursor/mcp.json` command/args/env
into `approvedProjectMcpServers`. Changing mcp.json `env` invalidates that hash
and Cursor auto-disables the project server until you re-enable it in the UI.

This script:
  1. Removes `project-0-*-marengo-pi` from every marengo workspace disabled list
  2. Re-approves the current mcp.json hash for known marengo workspaces

Prefer keeping profile/SSH defaults in `tools/marengo-pi-mcp/run-mcp.sh` (not
mcp.json) so the approval hash stays stable.

Close Cursor (or at least fully quit) before running with --write when possible,
otherwise the live state DB may race the WAL. Dry-run is the default.
sessionStart hooks may pass --write --best-effort to soft-fail on locks.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sqlite3
import sys
import tempfile
from pathlib import Path
from typing import Any

SERVER_SUFFIX = "marengo-pi"
DISABLED_KEY = "cursor/disabledMcpServers"
APPROVED_KEY = "cursor/approvedProjectMcpServers"
# Also used by Cursor's storage service without the cursor/ prefix in some paths.
DISABLED_KEY_ALT = "disabledMcpServers"
APPROVED_KEY_ALT = "approvedProjectMcpServers"


def y_le(e: int, t: int) -> int:
    """Match JS `(t<<5)-t+e|0` (signed int32)."""
    r = ((t << 5) - t + e) & 0xFFFFFFFF
    if r >= 0x80000000:
        r -= 0x100000000
    return r


def string_hash(e: str, t: int = 0) -> int:
    """Match Cursor's SWe / zy string hash (signed 32-bit)."""
    t = y_le(149417, t)
    for ch in e:
        t = y_le(ord(ch), t)
    return t


def config_hash(server: dict[str, Any]) -> str:
    keys = ("command", "args", "env", "envFile", "url", "headers")
    payload = {k: server[k] for k in keys if k in server}
    # JS JSON.stringify: no spaces; key order = insertion order.
    h = string_hash(json.dumps(payload, separators=(",", ":"), ensure_ascii=True))
    return format(h, "x")[:16]


def approval_key(identifier: str, server: dict[str, Any]) -> str:
    return f"{identifier}:{config_hash(server)}"


def resolve_mcp_server(mcp_json: Path, workspace_folder: str) -> dict[str, Any]:
    raw = json.loads(mcp_json.read_text(encoding="utf-8"))
    server = raw["mcpServers"]["marengo-pi"]
    folder = workspace_folder.rstrip("/\\")

    def resolve(value: Any) -> Any:
        if isinstance(value, str):
            return value.replace("${workspaceFolder}", folder)
        if isinstance(value, list):
            return [resolve(v) for v in value]
        if isinstance(value, dict):
            return {k: resolve(v) for k, v in value.items()}
        return value

    return resolve(server)


def windows_cursor_root() -> Path | None:
    appdata = os.environ.get("APPDATA")
    if appdata:
        p = Path(appdata) / "Cursor" / "User"
        if p.is_dir():
            return p
    # WSL → Windows Cursor
    candidates = [
        Path("/mnt/c/Users") / u / "AppData/Roaming/Cursor/User"
        for u in os.listdir("/mnt/c/Users")
        if not u.startswith(".") and (Path("/mnt/c/Users") / u / "AppData/Roaming/Cursor/User").is_dir()
    ] if Path("/mnt/c/Users").is_dir() else []
    for c in candidates:
        if (c / "globalStorage" / "state.vscdb").is_file():
            return c
    home = Path.home() / ".config" / "Cursor" / "User"
    if home.is_dir():
        return home
    return None


def is_marengo_workspace(workspace_json: Path) -> bool:
    try:
        text = workspace_json.read_text(encoding="utf-8", errors="ignore").lower()
    except OSError:
        return False
    return "marengo" in text


def project_identifier(workspace_basename: str = "marengo") -> str:
    # Cursor: project-0-<folderBasename>-<serverName>
    return f"project-0-{workspace_basename}-{SERVER_SUFFIX}"


def read_json_list(cur: sqlite3.Cursor, key: str) -> list[str]:
    row = cur.execute("SELECT value FROM ItemTable WHERE key=?", (key,)).fetchone()
    if not row:
        return []
    raw = row[0]
    if isinstance(raw, bytes):
        raw = raw.decode("utf-8", errors="replace")
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        return []
    return data if isinstance(data, list) else []


def write_json_list(cur: sqlite3.Cursor, key: str, values: list[str]) -> None:
    payload = json.dumps(values, separators=(",", ":"))
    exists = cur.execute("SELECT 1 FROM ItemTable WHERE key=?", (key,)).fetchone()
    if exists:
        cur.execute("UPDATE ItemTable SET value=? WHERE key=?", (payload, key))
    else:
        cur.execute("INSERT INTO ItemTable (key, value) VALUES (?, ?)", (key, payload))


def snapshot_db(src: Path) -> Path:
    tmp = Path(tempfile.mkdtemp(prefix="mcp-enable-")) / "state.vscdb"
    shutil.copy2(src, tmp)
    for suf in ("-wal", "-shm"):
        side = Path(str(src) + suf)
        if side.is_file():
            shutil.copy2(side, Path(str(tmp) + suf))
    con = sqlite3.connect(tmp)
    try:
        con.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    finally:
        con.close()
    return tmp


def scrub_disabled(disabled: list[str], identifier: str) -> list[str]:
    return [x for x in disabled if x != identifier and not x.startswith(f"{identifier}:")]


def merge_approval(approved: list[str], identifier: str, key: str) -> list[str]:
    # Drop stale hashes for this server, keep others, add current key + legacy bare id.
    kept = [
        x
        for x in approved
        if x != identifier and not (isinstance(x, str) and x.startswith(f"{identifier}:"))
    ]
    for item in (identifier, key):
        if item not in kept:
            kept.append(item)
    return kept


def process_workspace_db(
    db_path: Path,
    identifier: str,
    *,
    write: bool,
    best_effort: bool = False,
) -> tuple[list[str], list[str]]:
    snap = snapshot_db(db_path)
    con = sqlite3.connect(snap)
    cur = con.cursor()
    before = read_json_list(cur, DISABLED_KEY)
    if not before:
        before = read_json_list(cur, DISABLED_KEY_ALT)
    after = scrub_disabled(before, identifier)
    con.close()

    if write and after != before:
        try:
            live = sqlite3.connect(db_path)
            lcur = live.cursor()
            write_json_list(lcur, DISABLED_KEY, after)
            if read_json_list(lcur, DISABLED_KEY_ALT) is not None:
                # Keep alt key in sync when present.
                alt = read_json_list(lcur, DISABLED_KEY_ALT)
                if alt or DISABLED_KEY_ALT:
                    write_json_list(lcur, DISABLED_KEY_ALT, after)
            live.commit()
            live.close()
        except sqlite3.Error as exc:
            msg = f"could not write workspace disabled list ({db_path}: {exc})"
            if best_effort:
                print(f"warning: {msg}", file=sys.stderr)
                return before, before
            print(f"error: {msg}", file=sys.stderr)
            raise SystemExit(2) from exc
    return before, after


def process_global_approvals(
    global_db: Path,
    identifier: str,
    key: str,
    *,
    write: bool,
    best_effort: bool = False,
) -> tuple[list[str], list[str]]:
    snap = snapshot_db(global_db)
    con = sqlite3.connect(snap)
    cur = con.cursor()
    before = read_json_list(cur, APPROVED_KEY) or read_json_list(cur, APPROVED_KEY_ALT)
    after = merge_approval(before, identifier, key)
    con.close()

    if write and after != before:
        try:
            live = sqlite3.connect(global_db)
            lcur = live.cursor()
            write_json_list(lcur, APPROVED_KEY, after)
            write_json_list(lcur, APPROVED_KEY_ALT, after)
            live.commit()
            live.close()
        except sqlite3.Error as exc:
            msg = (
                f"could not write global approvals ({exc}). "
                "Fully quit Cursor and re-run with --write, or enable "
                "marengo-pi once in MCP settings (whitelist writes the new hash)."
            )
            if best_effort:
                print(f"warning: {msg}", file=sys.stderr)
                return before, before
            print(f"error: {msg}", file=sys.stderr)
            raise SystemExit(2) from exc
    return before, after


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write",
        action="store_true",
        help="Apply changes (default is dry-run). Quit Cursor first when possible.",
    )
    parser.add_argument(
        "--best-effort",
        action="store_true",
        help=(
            "With --write: soft-fail on locked DBs (for sessionStart hooks while "
            "Cursor is running). Still exits 0 when scrub/approve cannot complete."
        ),
    )
    parser.add_argument(
        "--repo",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Marengo repo root (for mcp.json)",
    )
    args = parser.parse_args()
    repo = args.repo.resolve()
    mcp_json = repo / ".cursor" / "mcp.json"
    if not mcp_json.is_file():
        print(f"error: missing {mcp_json}", file=sys.stderr)
        return 1

    cursor_user = windows_cursor_root()
    if cursor_user is None:
        print("error: could not find Cursor User data dir", file=sys.stderr)
        return 1

    identifier = project_identifier("marengo")
    server = resolve_mcp_server(mcp_json, str(repo))
    key = approval_key(identifier, server)

    print(f"Cursor User: {cursor_user}")
    print(f"Identifier:  {identifier}")
    print(f"Approval:    {key}")
    print(f"mcp.json:    {mcp_json}")
    print(f"Mode:        {'WRITE' if args.write else 'dry-run'}"
          f"{' (best-effort)' if args.best_effort else ''}")
    if args.write and not args.best_effort:
        print("NOTE: Cursor should be fully quit before --write.")

    ws_root = cursor_user / "workspaceStorage"
    changed = False
    for ws_dir in sorted(ws_root.iterdir() if ws_root.is_dir() else []):
        wj = ws_dir / "workspace.json"
        db = ws_dir / "state.vscdb"
        if not wj.is_file() or not db.is_file():
            continue
        if not is_marengo_workspace(wj):
            continue
        before, after = process_workspace_db(
            db, identifier, write=args.write, best_effort=args.best_effort
        )
        if before != after:
            changed = True
            print(f"\nworkspace {ws_dir.name}")
            print(f"  disabled before: {before}")
            print(f"  disabled after:  {after}")
        else:
            print(f"\nworkspace {ws_dir.name}: disabled list ok ({before})")

    global_db = cursor_user / "globalStorage" / "state.vscdb"
    if global_db.is_file():
        before, after = process_global_approvals(
            global_db,
            identifier,
            key,
            write=args.write,
            best_effort=args.best_effort,
        )
        if before != after:
            changed = True
            print("\nglobal approvals")
            print(f"  before: {before}")
            print(f"  after:  {after}")
        else:
            print(f"\nglobal approvals already include {key}")
    else:
        print(f"warning: missing {global_db}", file=sys.stderr)

    if not args.write and changed:
        print("\nDry-run only. Re-run with --write after quitting Cursor.")
    elif args.write:
        print("\nDone. Reload Cursor window (or reopen) so MCP picks up state.")
    else:
        print("\nNo changes needed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
