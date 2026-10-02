"""T17: public search handlers await cold sources and reuse their disk results."""

import json
from datetime import datetime, timezone

import pytest

from marengo_research_mcp.cache import ResearchCache
from marengo_research_mcp.config import Config
from marengo_research_mcp.models import ResearchHit
from marengo_research_mcp.tools import search


HANDLERS = [
    (search.search_github_tool, "search_github", "github"),
    (search.search_reddit_tool, "search_reddit", "reddit"),
    (search.search_web_tool, "search_duckduckgo", "web"),
    (search.search_forums_tool, "search_forums", "forums"),
    (search.search_vendor_docs_tool, "search_vendor_docs", "vendor"),
    (search.search_hf_tool, "search_huggingface", "hf"),
]


@pytest.mark.parametrize("handler,source,namespace", HANDLERS)
@pytest.mark.parametrize("source_fails", [False, True], ids=["hits", "source-error"])
async def test_public_handler_cold_and_disk_cache(
    tmp_path, monkeypatch, handler, source, namespace, source_fails
):
    cfg = Config(
        cache_dir=tmp_path,
        github_token=None,
        reddit_client_id=None,
        reddit_client_secret=None,
        max_scrape=5,
        scrape_timeout_s=1,
        cache_ttl_hours=24,
        max_concurrent_scrape=1,
        user_agent="offline-test",
    )
    awaited_calls = []
    stamp = datetime(2026, 1, 2, 3, 4, tzinfo=timezone.utc)
    hit = ResearchHit(
        type="code", title="Independent result", url="https://fixture.invalid/robot",
        source_name=namespace, published_at=stamp, updated_at=stamp,
        stars=42, authors=["A. Researcher"], year=2026,
    )

    async def provider(received_cfg, query, limit):
        awaited_calls.append((received_cfg, query, limit))
        if source_fails:
            raise RuntimeError("provider unavailable")
        return [hit]

    monkeypatch.setattr(search, source, provider)
    cold = json.loads(await handler(cfg, ResearchCache(cfg), "humanoid", 2))
    assert awaited_calls == [(cfg, "humanoid", 2)]
    assert cold["query"] == "humanoid" and cold["cached"] is False
    assert cold["hits"] == ([] if source_fails else [hit.model_dump(mode="json")])
    if not source_fails:
        returned = cold["hits"][0]
        assert returned["source_name"] == namespace
        assert returned["stars"] == 42 and returned["authors"] == ["A. Researcher"]
        assert returned["year"] == 2026
        assert returned["published_at"] == "2026-01-02T03:04:00Z"
        assert returned["updated_at"] == "2026-01-02T03:04:00Z"
    assert cold["errors"] == (
        [f"{namespace}: provider unavailable"] if source_fails else []
    )

    # A new cache object must read the actual persisted response without another call.
    hot = json.loads(await handler(cfg, ResearchCache(cfg), "humanoid", 2))
    assert hot == {**cold, "cached": True}
    assert awaited_calls == [(cfg, "humanoid", 2)]

    # A distinct limit is a separate request, not a hit on the prior response.
    changed = json.loads(await handler(cfg, ResearchCache(cfg), "humanoid", 3))
    assert changed["cached"] is False
    assert awaited_calls == [(cfg, "humanoid", 2), (cfg, "humanoid", 3)]
