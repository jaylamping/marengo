"""GitHub search API."""

from __future__ import annotations

import httpx

from marengo_research_mcp.config import Config
from marengo_research_mcp.models import ResearchHit

GITHUB_SEARCH = "https://api.github.com/search/repositories"


async def search_github(cfg: Config, query: str, limit: int = 10) -> list[ResearchHit]:
    q = f"{query} humanoid OR biped OR robotics in:name,description,readme"
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
            )
        )
    return hits
