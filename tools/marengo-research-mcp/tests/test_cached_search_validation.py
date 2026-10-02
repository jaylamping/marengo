"""Invalid normalized provider results must produce a public error response."""

import json
from dataclasses import replace

import pytest

from marengo_research_mcp.cache import ResearchCache
from marengo_research_mcp.config import load_config
from marengo_research_mcp.tools import search
from .test_cached_search_handlers import HANDLERS


@pytest.mark.parametrize("handler,source,namespace", HANDLERS)
async def test_invalid_provider_response_is_reported(tmp_path, monkeypatch, handler, source, namespace):
    cfg = replace(load_config(), cache_dir=tmp_path)
    calls = []

    async def invalid_provider(received_cfg, query, limit):
        calls.append((received_cfg, query, limit))
        return {"unexpected": "a mapping rather than a hit list"}

    monkeypatch.setattr(search, source, invalid_provider)
    cold = json.loads(await handler(cfg, ResearchCache(cfg), "humanoid", 2))
    assert cold["query"] == "humanoid" and cold["cached"] is False
    assert cold["hits"] == []
    assert len(cold["errors"]) == 1
    assert cold["errors"][0].startswith(f"{namespace}: ")
    assert "validation error" in cold["errors"][0]
    hot = json.loads(await handler(cfg, ResearchCache(cfg), "humanoid", 2))
    assert hot == {**cold, "cached": True}
    assert calls == [(cfg, "humanoid", 2)]


@pytest.mark.parametrize("handler,source,namespace", HANDLERS)
async def test_invalid_disk_response_is_refreshed(tmp_path, monkeypatch, handler, source, namespace):
    cfg = replace(load_config(), cache_dir=tmp_path)
    calls = []

    async def provider(*args):
        calls.append(args)
        return []

    monkeypatch.setattr(search, source, provider)
    first = json.loads(await handler(cfg, ResearchCache(cfg), "humanoid", 2))
    path, = (tmp_path / "search").glob("*.json")
    record = json.loads(path.read_text(encoding="utf-8"))
    record["value"]["hits"] = {"unexpected": "a mapping rather than a hit list"}
    path.write_text(json.dumps(record), encoding="utf-8")
    refreshed = json.loads(await handler(cfg, ResearchCache(cfg), "humanoid", 2))
    assert refreshed == first
    assert calls == [(cfg, "humanoid", 2), (cfg, "humanoid", 2)]
