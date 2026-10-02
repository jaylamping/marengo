"""A corrupted UTF-8 cache file cannot abort the public search response."""

import json
from dataclasses import replace

from marengo_research_mcp.cache import ResearchCache
from marengo_research_mcp.config import load_config
from marengo_research_mcp.tools import search


async def test_corrupt_cache_bytes_refresh_public_search(tmp_path, monkeypatch):
    cfg = replace(load_config(), cache_dir=tmp_path)
    calls = []

    async def provider(*args):
        calls.append(args)
        return []

    monkeypatch.setattr(search, "search_github", provider)
    first = json.loads(await search.search_github_tool(cfg, ResearchCache(cfg), "robot", 1))
    path, = (tmp_path / "search").glob("*.json")
    path.write_bytes(b"\xff")
    refreshed = json.loads(await search.search_github_tool(cfg, ResearchCache(cfg), "robot", 1))
    assert refreshed == first
    assert refreshed["hits"] == [] and refreshed["errors"] == []
    assert refreshed["cached"] is False
    assert calls == [(cfg, "robot", 1), (cfg, "robot", 1)]
