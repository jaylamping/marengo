"""A dated normalized result survives the real cold/hot disk cache path."""
from dataclasses import replace
from datetime import datetime, timezone
from unittest.mock import AsyncMock

from marengo_research_mcp.cache import ResearchCache
from marengo_research_mcp.config import load_config
from marengo_research_mcp.models import ResearchHit
from marengo_research_mcp.tools.search import _cached_search


async def test_dated_result_disk_cache_roundtrip(tmp_path):
    cfg = replace(load_config(), cache_dir=tmp_path)
    stamp = datetime.fromisoformat("2025-12-31T07:00:00-05:00")
    provider = AsyncMock(return_value=[ResearchHit(type="code", title="robot", url="https://fixture.invalid", published_at=stamp, updated_at=stamp)])
    cold = await _cached_search(ResearchCache(cfg), "fixture", "robot", 1, provider)
    hot = await _cached_search(ResearchCache(cfg), "fixture", "robot", 1, provider)
    assert not cold.cached and hot.cached
    assert hot.hits == cold.hits
    assert hot.hits[0].updated_at.utcoffset() == stamp.utcoffset()
    provider.assert_awaited_once()
