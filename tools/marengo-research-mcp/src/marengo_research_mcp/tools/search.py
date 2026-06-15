"""Granular search tool handlers."""

from __future__ import annotations

import asyncio
import json

from marengo_research_mcp.cache import ResearchCache
from marengo_research_mcp.config import Config
from marengo_research_mcp.models import SearchResponse
from marengo_research_mcp.sources.arxiv import search_arxiv
from marengo_research_mcp.sources.forums import search_forums
from marengo_research_mcp.sources.github import search_github
from marengo_research_mcp.sources.huggingface import search_huggingface
from marengo_research_mcp.sources.openreview import search_openreview
from marengo_research_mcp.sources.papers_with_code import search_papers_with_code
from marengo_research_mcp.sources.reddit import search_reddit
from marengo_research_mcp.sources.semantic_scholar import search_semantic_scholar
from marengo_research_mcp.sources.standards import search_standards
from marengo_research_mcp.sources.vendor_docs import search_vendor_docs
from marengo_research_mcp.sources.web import search_duckduckgo


async def _cached_search(
    cache: ResearchCache,
    name: str,
    query: str,
    limit: int,
    fn,
) -> SearchResponse:
    key = json.dumps({"q": query, "limit": limit})
    cached = cache.get("search", f"{name}:{key}")
    if cached is not None:
        return SearchResponse.model_validate({**cached, "cached": True})
    errors: list[str] = []
    hits = []
    try:
        if asyncio.iscoroutinefunction(fn):
            hits = await fn(query, limit)
        else:
            hits = fn(query, limit)
    except Exception as exc:
        errors.append(f"{name}: {exc}")
    resp = SearchResponse(query=query, hits=hits, errors=errors)
    cache.set("search", f"{name}:{key}", resp.model_dump())
    return resp


async def search_papers(cfg: Config, cache: ResearchCache, query: str, limit: int = 10) -> str:
    async def s2(q: str, lim: int):
        return await search_semantic_scholar(cfg, q, lim)

    async def orv(q: str, lim: int):
        return await search_openreview(cfg, q, lim)

    async def pwc(q: str, lim: int):
        return await search_papers_with_code(cfg, q, lim)

    per = max(limit // 4, 2)
    parts = await asyncio.gather(
        asyncio.to_thread(search_arxiv, query, per),
        s2(query, per),
        orv(query, per),
        pwc(query, per),
        return_exceptions=True,
    )
    hits = []
    errors = []
    for i, part in enumerate(parts):
        if isinstance(part, Exception):
            errors.append(str(part))
        else:
            hits.extend(part)
    resp = SearchResponse(query=query, hits=hits[:limit], errors=errors)
    return resp.model_dump_json(indent=2)


async def search_github_tool(cfg: Config, cache: ResearchCache, query: str, limit: int = 10) -> str:
    resp = await _cached_search(
        cache,
        "github",
        query,
        limit,
        lambda q, lim: search_github(cfg, q, lim),
    )
    return resp.model_dump_json(indent=2)


async def search_reddit_tool(cfg: Config, cache: ResearchCache, query: str, limit: int = 10) -> str:
    resp = await _cached_search(
        cache,
        "reddit",
        query,
        limit,
        lambda q, lim: search_reddit(cfg, q, lim),
    )
    return resp.model_dump_json(indent=2)


async def search_web_tool(cfg: Config, cache: ResearchCache, query: str, limit: int = 10) -> str:
    resp = await _cached_search(
        cache,
        "web",
        query,
        limit,
        lambda q, lim: search_duckduckgo(cfg, q, lim),
    )
    return resp.model_dump_json(indent=2)


async def search_forums_tool(cfg: Config, cache: ResearchCache, query: str, limit: int = 10) -> str:
    resp = await _cached_search(
        cache,
        "forums",
        query,
        limit,
        lambda q, lim: search_forums(cfg, q, lim),
    )
    return resp.model_dump_json(indent=2)


async def search_vendor_docs_tool(
    cfg: Config, cache: ResearchCache, query: str, limit: int = 10
) -> str:
    resp = await _cached_search(
        cache,
        "vendor",
        query,
        limit,
        lambda q, lim: search_vendor_docs(cfg, q, lim),
    )
    return resp.model_dump_json(indent=2)


async def search_hf_tool(cfg: Config, cache: ResearchCache, query: str, limit: int = 10) -> str:
    resp = await _cached_search(
        cache,
        "hf",
        query,
        limit,
        lambda q, lim: search_huggingface(cfg, q, lim),
    )
    return resp.model_dump_json(indent=2)


async def search_standards_tool(query: str, limit: int = 10) -> str:
    hits = search_standards(query, limit)
    return SearchResponse(query=query, hits=hits).model_dump_json(indent=2)
