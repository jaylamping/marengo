"""ROS Discourse and Stack Exchange search."""

from __future__ import annotations

import httpx

from marengo_research_mcp.config import Config
from marengo_research_mcp.models import ResearchHit
from marengo_research_mcp.sources.web import search_duckduckgo

STACK_EXCHANGE = "https://api.stackexchange.com/2.3/search/advanced"


async def search_ros_discourse(cfg: Config, query: str, limit: int = 5) -> list[ResearchHit]:
    ddg_hits = await search_duckduckgo(
        cfg, f"site:discourse.ros.org {query}", limit=limit
    )
    for h in ddg_hits:
        h.type = "forum"
        h.source_name = "ros_discourse"
    return ddg_hits


async def search_stack_exchange(cfg: Config, query: str, limit: int = 5) -> list[ResearchHit]:
    params = {
        "order": "desc",
        "sort": "relevance",
        "q": query,
        "site": "robotics",
        "pagesize": limit,
    }
    headers = {"User-Agent": cfg.user_agent}
    hits: list[ResearchHit] = []
    try:
        async with httpx.AsyncClient(timeout=30.0, headers=headers) as client:
            resp = await client.get(STACK_EXCHANGE, params=params)
            if resp.status_code != 200:
                return hits
            data = resp.json()
    except httpx.HTTPError:
        return hits

    for item in data.get("items", []):
        hits.append(
            ResearchHit(
                type="forum",
                title=item.get("title", ""),
                url=item.get("link", ""),
                snippet="",
                source_name="stackexchange/robotics",
            )
        )

    if len(hits) < limit:
        params["site"] = "stackoverflow"
        try:
            async with httpx.AsyncClient(timeout=30.0, headers=headers) as client:
                resp = await client.get(STACK_EXCHANGE, params=params)
                if resp.status_code == 200:
                    for item in resp.json().get("items", []):
                        hits.append(
                            ResearchHit(
                                type="forum",
                                title=item.get("title", ""),
                                url=item.get("link", ""),
                                snippet="",
                                source_name="stackexchange/stackoverflow",
                            )
                        )
        except httpx.HTTPError:
            pass
    return hits[:limit]


async def search_forums(cfg: Config, query: str, limit: int = 10) -> list[ResearchHit]:
    half = max(limit // 2, 1)
    ros = await search_ros_discourse(cfg, query, limit=half)
    se = await search_stack_exchange(cfg, query, limit=limit - len(ros))
    return (ros + se)[:limit]
