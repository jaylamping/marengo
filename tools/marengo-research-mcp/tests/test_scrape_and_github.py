import json
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from marengo_research_mcp.config import Config
from marengo_research_mcp.models import ResearchHit
from marengo_research_mcp.scrape.extract import extract_markdown


@pytest.fixture
def cfg(tmp_path):
    return Config(
        cache_dir=tmp_path,
        github_token=None,
        reddit_client_id=None,
        reddit_client_secret=None,
        max_scrape=3,
        scrape_timeout_s=5.0,
        cache_ttl_hours=24,
        max_concurrent_scrape=2,
        user_agent="test-agent",
    )


def test_extract_markdown_trafilatura():
    html = "<html><body><article><h1>Title</h1><p>Content " + ("word " * 50) + "</p></article></body></html>"
    text, source = extract_markdown(html, "https://example.com")
    assert source in ("trafilatura", "markdownify")
    assert len(text) >= 200


@pytest.mark.asyncio
async def test_search_github_parses_response(cfg):
    from marengo_research_mcp.sources.github import search_github

    mock_response = {
        "items": [
            {
                "full_name": "org/humanoid",
                "html_url": "https://github.com/org/humanoid",
                "description": "biped robot",
                "stargazers_count": 100,
            }
        ]
    }
    with patch("marengo_research_mcp.sources.github.httpx.AsyncClient") as client_cls:
        client = AsyncMock()
        client_cls.return_value.__aenter__.return_value = client
        resp = MagicMock()
        resp.status_code = 200
        resp.json.return_value = mock_response
        client.get = AsyncMock(return_value=resp)
        hits = await search_github(cfg, "humanoid", limit=5)
    assert len(hits) == 1
    assert hits[0].type == "code"
    assert hits[0].stars == 100
