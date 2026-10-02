"""Cache version and timestamps qualify reuse independently of response shape."""

import json
from dataclasses import replace

import pytest

from marengo_research_mcp import cache as module
from marengo_research_mcp.config import load_config


@pytest.mark.parametrize("record,reusable", [
    ({"schema_version": 1, "ts": 0, "value": {"item": "kept"}}, True),
    ({"schema_version": 1, "ts": -0.001, "value": {"item": "kept"}}, False),
    ({"schema_version": 1, "ts": 3600.001, "value": {"item": "kept"}}, False),
    ({"schema_version": 2, "ts": 3600, "value": {"item": "kept"}}, False),
    ({"ts": 3600, "value": {"item": "kept"}}, False),
    ({"schema_version": 1, "value": {"item": "kept"}}, False),
    ({"schema_version": 1, "ts": "3600", "value": {"item": "kept"}}, False),
    ({"schema_version": 1, "ts": True, "value": {"item": "kept"}}, False),
    ({"schema_version": 1, "ts": float("nan"), "value": {"item": "kept"}}, False),
    ({"schema_version": 1, "ts": float("inf"), "value": {"item": "kept"}}, False),
    ([], False),
])
def test_unqualified_record_is_a_miss(tmp_path, monkeypatch, record, reusable):
    monkeypatch.setattr(module.time, "time", lambda: 3600)
    cfg = replace(load_config(), cache_dir=tmp_path, cache_ttl_hours=1)
    cache = module.ResearchCache(cfg)
    cache.set("search", "fixture", {"item": "kept"})
    path, = (tmp_path / "search").glob("*.json")
    # Independent on-disk inputs, including the exact inclusive expiry boundary.
    path.write_text(json.dumps(record), encoding="utf-8")
    before = path.read_bytes()
    assert module.ResearchCache(cfg).get("search", "fixture") == (
        {"item": "kept"} if reusable else None
    )
    assert path.read_bytes() == before


def test_written_record_survives_reopen_and_then_expires(tmp_path, monkeypatch):
    now = [1000]
    monkeypatch.setattr(module.time, "time", lambda: now[0])
    cfg = replace(load_config(), cache_dir=tmp_path, cache_ttl_hours=1)
    module.ResearchCache(cfg).set("scrape", "fixture", {"content": "saved text"})
    now[0] = 4600
    assert module.ResearchCache(cfg).get("scrape", "fixture") == {"content": "saved text"}
    now[0] = 4600.001
    assert module.ResearchCache(cfg).get("scrape", "fixture") is None
