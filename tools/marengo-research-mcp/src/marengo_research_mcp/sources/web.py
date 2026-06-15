"""DuckDuckGo web search."""

from __future__ import annotations

from duckduckgo_search import DDGS

from marengo_research_mcp.config import Config
from marengo_research_mcp.models import ResearchHit


async def search_duckduckgo(cfg: Config, query: str, limit: int = 10) -> list[ResearchHit]:
    del cfg  # reserved for future proxy/user-agent config
    hits: list[ResearchHit] = []
    try:
        with DDGS() as ddgs:
            for r in ddgs.text(query, max_results=limit):
                hits.append(
                    ResearchHit(
                        type="web",
                        title=r.get("title", ""),
                        url=r.get("href", r.get("link", "")),
                        snippet=(r.get("body") or r.get("snippet") or "")[:500],
                        source_name="duckduckgo",
                    )
                )
    except Exception:
        pass
    return hits
