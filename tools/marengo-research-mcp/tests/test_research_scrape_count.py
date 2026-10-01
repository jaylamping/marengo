"""Public orchestrator scrape counts, with all network boundaries replaced."""

import inspect
from dataclasses import replace
from unittest.mock import AsyncMock

import pytest

from marengo_research_mcp.cache import ResearchCache
from marengo_research_mcp.config import load_config
from marengo_research_mcp.models import ResearchHit, ResearchHumanoidResponse
from marengo_research_mcp.server import list_tools
from marengo_research_mcp.tools import research


@pytest.mark.parametrize("requested, expected", [("omitted", 3), (None, 3), (0, 0), (2, 2), (99, 4), (-1, 0)])
async def test_public_scrape_count(tmp_path, monkeypatch, requested, expected):
    cfg = replace(load_config(), cache_dir=tmp_path, max_scrape=4)
    cache = ResearchCache(cfg)
    hits = [ResearchHit(type="web", title=f"robot {i}", url=f"https://example.test/{i}") for i in range(6)]
    provider = AsyncMock(return_value=(hits, None))
    scraper = AsyncMock(return_value={"content": "article"})
    monkeypatch.setattr(research, "FOCUS_SOURCES", {"all": ["offline"]})
    monkeypatch.setattr(research, "_run_source", provider)
    monkeypatch.setattr(research, "scrape_url", scraper)
    kwargs = {} if requested == "omitted" else {"scrape_top_n": requested}
    response = ResearchHumanoidResponse.model_validate_json(
        await research.research_humanoid(cfg, cache, "robot", **kwargs)
    )
    assert len(response.hits) == 6
    assert scraper.await_count == expected
    assert len(list((tmp_path / "scrape").glob("*.json"))) == expected
    assert sum(hit.scraped_markdown == "article" for hit in response.hits) == expected
    provider.assert_awaited_once()


async def test_schema_and_function_default_match():
    tool = next(tool for tool in await list_tools() if tool.name == "research_humanoid")
    schema_default = tool.inputSchema["properties"]["scrape_top_n"]["default"]
    assert schema_default == inspect.signature(research.research_humanoid).parameters["scrape_top_n"].default == 3
