"""Provider payloads flow through actual normalization and public JSON output."""
from dataclasses import replace
from datetime import datetime, timezone
from unittest.mock import AsyncMock

import httpx
import pytest

from marengo_research_mcp.cache import ResearchCache
from marengo_research_mcp.config import load_config
from marengo_research_mcp.models import ResearchHumanoidResponse
from marengo_research_mcp.tools import research

NOW = datetime(2026, 1, 2, 12, tzinfo=timezone.utc)
STAMP = "2025-12-31T07:00:00-05:00"
BASE = {"title": "robot paper", "url": "https://fixture.invalid/paper"}


@pytest.mark.parametrize("source,payload", [
    ("github", {"items": [{"full_name": "robot/repo", "html_url": "https://fixture.invalid/repo", "created_at": "2020-01-01T00:00:00Z", "pushed_at": STAMP}]}),
    ("semantic_scholar", {"data": [dict(BASE, publicationDate="2025-12-31")]}),
    ("openreview", {"notes": [{"id": "robot", "content": {"title": "robot paper"}, "odate": 1767182400000, "pdate": 1735689600000}]}),
    ("papers_with_code", {"results": [{"title": "robot paper", "url_abs": "https://fixture.invalid/paper", "published": "2025-12-31"}]}),
    ("reddit", {"data": {"children": [{"data": dict(BASE, created_utc=1767182400)}]}}),
    ("huggingface", [{"id": "robot/model", "pipeline_tag": "robotics", "createdAt": "2020-01-01T00:00:00Z", "lastModified": STAMP}]),
])
async def test_real_provider_dates_reach_public_window(tmp_path, monkeypatch, source, payload):
    cfg = replace(load_config(), cache_dir=tmp_path, github_token=None)
    requests = []

    class Client:
        def __init__(self, **kwargs):
            pass
        async def __aenter__(self):
            return self
        async def __aexit__(self, *args):
            pass
        async def get(self, url, **kwargs):
            requests.append((url, kwargs))
            return httpx.Response(200, json=payload, request=httpx.Request("GET", url))

    monkeypatch.setattr(httpx, "AsyncClient", Client)
    monkeypatch.setattr(research, "FOCUS_SOURCES", {"all": [source]})
    monkeypatch.setattr(research, "_utc_now", lambda: NOW)
    # Reddit has a fallback for incomplete provider result sets; this fixture requests one.
    response = ResearchHumanoidResponse.model_validate_json(await research.research_humanoid(
        cfg, ResearchCache(cfg), "robot", max_results_per_source=1, scrape_top_n=0, recency="week"))
    assert requests
    if source == "huggingface":
        assert requests[0][1]["params"]["expand"] == ["createdAt", "lastModified", "likes", "pipeline_tag"]
        assert requests[1][1]["params"]["expand"] == ["createdAt", "lastModified", "likes"]
    if source == "github":
        assert requests[0][1]["params"]["q"] == "robot in:name,description,readme pushed:2025-12-26..2026-01-02"
    if source == "semantic_scholar":
        assert requests[0][1]["params"]["publicationDateOrYear"] == "2025-12-26:2026-01-02"
    assert not response.errors
    assert len(response.hits) == 1
    if source == "huggingface":
        assert response.hits[0].snippet == "HF model: robotics"
    stamp = response.hits[0].updated_at if source in {"github", "huggingface"} else response.hits[0].published_at
    assert stamp.astimezone(timezone.utc) == datetime(2025, 12, 31, 12 if source in {"github", "huggingface", "openreview", "reddit"} else 0, tzinfo=timezone.utc)
    assert "unknown and future dates excluded" in response.summary


@pytest.mark.parametrize("window,days", [("week", 7), ("month", 30), ("year", 365)])
async def test_public_response_qualifies_boundaries(tmp_path, monkeypatch, window, days):
    from datetime import timedelta
    from marengo_research_mcp.models import ResearchHit

    cutoff = NOW - timedelta(days=days)
    dates = {"boundary": cutoff, "now": NOW,
             "outside": cutoff - timedelta(microseconds=1),
             "future": NOW + timedelta(microseconds=1), "unknown": None,
             "year-old": NOW - timedelta(days=366)}
    values = [ResearchHit(type="paper", title=name, url=f"https://fixture.invalid/{name}",
                          published_at=stamp) for name, stamp in dates.items()]
    provider = AsyncMock(return_value=(values, None))
    monkeypatch.setattr(research, "_run_source", provider)
    monkeypatch.setattr(research, "FOCUS_SOURCES", {"all": ["offline"]})
    monkeypatch.setattr(research, "_utc_now", lambda: NOW)
    cfg = replace(load_config(), cache_dir=tmp_path)
    result = ResearchHumanoidResponse.model_validate_json(await research.research_humanoid(
        cfg, ResearchCache(cfg), "robot", recency=window, scrape_top_n=0))
    assert {hit.title for hit in result.hits} == {"boundary", "now"}
    assert not result.errors


async def test_arxiv_dates_and_native_range_reach_public_response(tmp_path, monkeypatch):
    from types import SimpleNamespace
    from marengo_research_mcp.sources import arxiv as adapter
    searches = []
    stamp = datetime.fromisoformat(STAMP)
    class Search:
        def __init__(self, **kwargs):
            searches.append(kwargs)
    class Client:
        def results(self, search):
            return iter([SimpleNamespace(published=stamp, updated=NOW, title="robot paper",
                        entry_id="https://fixture.invalid/arxiv", summary="robot",
                        authors=[], pdf_url=None)])
    monkeypatch.setattr(adapter.arxiv, "Search", Search)
    monkeypatch.setattr(adapter.arxiv, "Client", Client)
    monkeypatch.setattr(research, "FOCUS_SOURCES", {"all": ["arxiv"]})
    calls = []
    def clock():
        calls.append(True)
        return NOW
    monkeypatch.setattr(research, "_utc_now", clock)
    cfg = replace(load_config(), cache_dir=tmp_path)
    response = ResearchHumanoidResponse.model_validate_json(await research.research_humanoid(
        cfg, ResearchCache(cfg), "robot", recency="week", scrape_top_n=0))
    assert len(response.hits) == 1
    assert response.hits[0].published_at == stamp
    assert response.hits[0].updated_at == NOW
    assert "submittedDate:[202512261200 TO 202601021200]" in searches[0]["query"]
    assert len(calls) == 1  # Provider query and local filtering share one captured clock.
