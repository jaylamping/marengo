"""JSON file cache for search and scrape results."""

from __future__ import annotations

import hashlib
import json
import math
import time
from pathlib import Path
from typing import Any

from marengo_research_mcp.config import Config

CACHE_SCHEMA_VERSION = 1


def _key(namespace: str, payload: str) -> str:
    digest = hashlib.sha256(f"{namespace}:{payload}".encode()).hexdigest()[:16]
    return digest


class ResearchCache:
    def __init__(self, cfg: Config) -> None:
        self.cfg = cfg
        self.root = cfg.cache_dir
        self.root.mkdir(parents=True, exist_ok=True)
        (self.root / "search").mkdir(exist_ok=True)
        (self.root / "scrape").mkdir(exist_ok=True)

    def _path(self, namespace: str, payload: str) -> Path:
        return self.root / namespace / f"{_key(namespace, payload)}.json"

    def get(self, namespace: str, payload: str) -> Any | None:
        path = self._path(namespace, payload)
        if not path.exists():
            return None
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, UnicodeDecodeError, OSError):
            return None
        if not isinstance(data, dict):
            return None
        version = data.get("schema_version")
        if type(version) is not int or version != CACHE_SCHEMA_VERSION:
            return None
        stamp = data.get("ts")
        if type(stamp) not in (int, float):
            return None
        try:
            if not math.isfinite(stamp):
                return None
            age_h = (time.time() - stamp) / 3600.0
        except OverflowError:
            return None
        if age_h < 0 or age_h > self.cfg.cache_ttl_hours:
            return None
        return data.get("value")

    def set(self, namespace: str, payload: str, value: Any) -> None:
        path = self._path(namespace, payload)
        path.write_text(
            json.dumps(
                {"schema_version": CACHE_SCHEMA_VERSION, "ts": time.time(), "value": value},
                ensure_ascii=False, indent=2,
            ),
            encoding="utf-8",
        )

    def stats(self) -> dict[str, int]:
        counts: dict[str, int] = {}
        for sub in ("search", "scrape"):
            d = self.root / sub
            counts[sub] = len(list(d.glob("*.json"))) if d.exists() else 0
        return counts
