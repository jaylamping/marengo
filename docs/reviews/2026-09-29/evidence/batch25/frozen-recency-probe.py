"""Public recency policy with unknown publication dates and no network access."""
from dataclasses import replace
from unittest.mock import AsyncMock

import pytest
from marengo_research_mcp.cache import ResearchCache
from marengo_research_mcp.config import load_config
from marengo_research_mcp.models import ResearchHit, ResearchHumanoidResponse
from marengo_research_mcp.tools import research

@pytest.mark.parametrize("recency, expected", [("week", 0), ("month", 0), ("year", 0), ("any", 1)])
async def test_requested_window_never_claims_unknown_date_is_recent(tmp_path, monkeypatch, recency, expected):
    cfg = replace(load_config(), cache_dir=tmp_path)
    cache = ResearchCache(cfg)
    provider = AsyncMock(return_value=([ResearchHit(type="paper", title="robot unknown date", url="https://example.test/unknown")], None))
    monkeypatch.setattr(research, "FOCUS_SOURCES", {"all": ["offline"]})
    monkeypatch.setattr(research, "_run_source", provider)
    response = ResearchHumanoidResponse.model_validate_json(await research.research_humanoid(
        cfg, cache, "robot", recency=recency, scrape_top_n=0,
    ))
    provider.assert_awaited_once()
    assert len(response.hits) == expected, "unknown date must not be represented as satisfying a requested window"
