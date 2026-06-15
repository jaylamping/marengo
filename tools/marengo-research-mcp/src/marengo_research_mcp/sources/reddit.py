"""Reddit search via public JSON API with DDG fallback."""

from __future__ import annotations

import httpx

from marengo_research_mcp.config import Config
from marengo_research_mcp.models import ResearchHit
from marengo_research_mcp.sources.web import search_duckduckgo

SUBREDDITS = ["robotics", "humanoidrobots", "ROS", "embedded", "humanoidrobotics"]
REDDIT_SEARCH = "https://www.reddit.com/search.json"


async def search_reddit(cfg: Config, query: str, limit: int = 10) -> list[ResearchHit]:
    hits: list[ResearchHit] = []
    sub_filter = " OR ".join(f"subreddit:{s}" for s in SUBREDDITS)
    params = {"q": f"{query} ({sub_filter})", "limit": min(limit, 25), "sort": "relevance"}
    headers = {"User-Agent": cfg.user_agent}
    try:
        async with httpx.AsyncClient(timeout=30.0, headers=headers, follow_redirects=True) as client:
            resp = await client.get(REDDIT_SEARCH, params=params)
            if resp.status_code == 200:
                data = resp.json()
                for child in data.get("data", {}).get("children", [])[:limit]:
                    post = child.get("data", {})
                    url = post.get("url", "")
                    permalink = post.get("permalink", "")
                    if permalink and not url.startswith("http"):
                        url = f"https://www.reddit.com{permalink}"
                    hits.append(
                        ResearchHit(
                            type="community",
                            title=post.get("title", ""),
                            url=url,
                            snippet=(post.get("selftext") or "")[:500],
                            source_name=f"reddit/r/{post.get('subreddit', '')}",
                        )
                    )
    except httpx.HTTPError:
        pass

    if len(hits) < limit:
        ddg = await search_duckduckgo(cfg, f"site:reddit.com {query}", limit=limit - len(hits))
        for h in ddg:
            h.type = "community"
            h.source_name = "reddit/ddg"
            hits.append(h)
    return hits[:limit]
