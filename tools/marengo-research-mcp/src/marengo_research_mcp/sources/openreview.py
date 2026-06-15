"""OpenReview search (notes API)."""

from __future__ import annotations

import httpx

from marengo_research_mcp.config import Config
from marengo_research_mcp.models import ResearchHit

OPENREVIEW_SEARCH = "https://api.openreview.net/notes/search"


async def search_openreview(cfg: Config, query: str, limit: int = 10) -> list[ResearchHit]:
    params = {
        "term": query,
        "content.venue": "CoRL,RSS,ICRA",
        "limit": limit,
        "source": "forum",
    }
    headers = {"User-Agent": cfg.user_agent}
    hits: list[ResearchHit] = []
    try:
        async with httpx.AsyncClient(timeout=30.0, headers=headers) as client:
            resp = await client.get(OPENREVIEW_SEARCH, params=params)
            if resp.status_code != 200:
                return hits
            data = resp.json()
    except httpx.HTTPError:
        return hits

    for note in data.get("notes", []):
        content = note.get("content") or {}
        title = content.get("title", "")
        if isinstance(title, dict):
            title = title.get("value", "")
        abstract = content.get("abstract", "")
        if isinstance(abstract, dict):
            abstract = abstract.get("value", "")
        forum = note.get("forum") or note.get("id", "")
        url = f"https://openreview.net/forum?id={forum}" if forum else ""
        if not title or not url:
            continue
        hits.append(
            ResearchHit(
                type="paper",
                title=str(title),
                url=url,
                snippet=str(abstract)[:500],
                source_name="openreview",
            )
        )
    return hits
