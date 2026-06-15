"""Scrape tool handlers."""

from __future__ import annotations

import json

from marengo_research_mcp.cache import ResearchCache
from marengo_research_mcp.config import Config
from marengo_research_mcp.scrape.extract import scrape_url, scrape_urls


async def scrape_url_tool(cfg: Config, cache: ResearchCache, url: str) -> str:
    cached = cache.get("scrape", url)
    if cached is not None:
        return json.dumps({**cached, "cached": True}, indent=2)
    result = await scrape_url(cfg, url)
    cache.set("scrape", url, result)
    return json.dumps(result, indent=2)


async def scrape_urls_tool(cfg: Config, cache: ResearchCache, urls: list[str]) -> str:
    capped = urls[: cfg.max_scrape]
    results = await scrape_urls(cfg, capped)
    for r in results:
        if r.get("content"):
            cache.set("scrape", r["url"], r)
    return json.dumps({"urls": capped, "results": results}, indent=2)
