"""GitHub search API."""

from __future__ import annotations

from datetime import datetime

import httpx

from marengo_research_mcp.config import Config
from marengo_research_mcp.dates import provider_datetime
from marengo_research_mcp.models import ResearchHit

GITHUB_SEARCH = "https://api.github.com/search/repositories"


async def search_github(cfg: Config, query: str, limit: int = 10, *, window: tuple[datetime, datetime] | None = None) -> list[ResearchHit]:
    q = f"{query} humanoid OR biped OR robotics in:name,description,readme"
    if window is not None:
        start, end = window
        # Whole UTC dates include the exact interval for subsequent local filtering.
        q = f"({q}) pushed:{start:%Y-%m-%d}..{end:%Y-%m-%d}"
    headers = {
        "Accept": "application/vnd.github+json",
        "User-Agent": cfg.user_agent,
    }
    if cfg.github_token:
        headers["Authorization"] = f"Bearer {cfg.github_token}"
    params = {"q": q, "sort": "stars", "order": "desc", "per_page": min(limit, 30)}
    hits: list[ResearchHit] = []
    try:
        async with httpx.AsyncClient(timeout=30.0, headers=headers) as client:
            resp = await client.get(GITHUB_SEARCH, params=params)
            if resp.status_code != 200:
                return hits
            data = resp.json()
    except httpx.HTTPError:
        return hits

    for repo in data.get("items", [])[:limit]:
        hits.append(
            ResearchHit(
                type="code",
                title=repo.get("full_name", ""),
                url=repo.get("html_url", ""),
                snippet=(repo.get("description") or "")[:500],
                source_name="github",
                stars=repo.get("stargazers_count"),
                published_at=provider_datetime(repo.get("created_at")),
                updated_at=provider_datetime(repo.get("pushed_at")),
            )
        )
    return hits
