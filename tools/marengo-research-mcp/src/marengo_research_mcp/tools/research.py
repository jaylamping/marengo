"""research_humanoid orchestrator."""

from __future__ import annotations

import asyncio
from datetime import datetime, timedelta, timezone
from typing import Literal

from marengo_research_mcp.cache import ResearchCache
from marengo_research_mcp.config import Config
from marengo_research_mcp.humanoid.queries import FOCUS_SOURCES, expand_query
from marengo_research_mcp.humanoid.rank import rank_hits
from marengo_research_mcp.models import ResearchHit, ResearchHumanoidResponse
from marengo_research_mcp.scrape.extract import scrape_url
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

Focus = Literal["papers", "code", "community", "vendor", "standards", "all"]
Recency = Literal["year", "month", "week", "any"]
DEFAULT_SCRAPE_TOP_N = 3
RECENCY_DAYS = {"week": 7, "month": 30, "year": 365}


async def _run_source(
    cfg: Config, name: str, query: str, per_source: int
) -> tuple[list[ResearchHit], str | None]:
    try:
        if name == "arxiv":
            return await asyncio.to_thread(search_arxiv, query, per_source), None
        if name == "semantic_scholar":
            return await search_semantic_scholar(cfg, query, per_source), None
        if name == "openreview":
            return await search_openreview(cfg, query, per_source), None
        if name == "papers_with_code":
            return await search_papers_with_code(cfg, query, per_source), None
        if name == "github":
            return await search_github(cfg, query, per_source), None
        if name == "reddit":
            return await search_reddit(cfg, query, per_source), None
        if name == "forums":
            return await search_forums(cfg, query, per_source), None
        if name == "vendor_docs":
            return await search_vendor_docs(cfg, query, per_source), None
        if name == "huggingface":
            return await search_huggingface(cfg, query, per_source), None
        if name == "standards":
            return search_standards(query, per_source), None
        if name == "web":
            return await search_duckduckgo(cfg, query, per_source), None
        return [], f"unknown source {name}"
    except Exception as exc:
        return [], f"{name}: {exc}"


def _utc_now() -> datetime:
    return datetime.now(timezone.utc)


def _filter_recency(
    hits: list[ResearchHit], recency: Recency, *, now: datetime | None = None
) -> list[ResearchHit]:
    if recency == "any":
        return hits
    current = now if now is not None else _utc_now()
    if current.tzinfo is None or current.utcoffset() is None:
        raise ValueError("recency clock must include a timezone")
    current = current.astimezone(timezone.utc)
    cutoff = current - timedelta(days=RECENCY_DAYS[recency])
    kept = []
    for hit in hits:
        # Code and Hub artifacts use activity; papers/posts use publication.
        stamp = (hit.updated_at or hit.published_at) if hit.type in {"code", "hf"} else hit.published_at
        if stamp is not None and cutoff <= stamp.astimezone(timezone.utc) <= current:
            kept.append(hit)
    return kept


async def research_humanoid(
    cfg: Config,
    cache: ResearchCache,
    query: str,
    focus: Focus = "all",
    max_results_per_source: int = 5,
    scrape_top_n: int | None = DEFAULT_SCRAPE_TOP_N,
    recency: Recency = "any",
) -> str:
    expanded = expand_query(query)
    search_query = expanded[0] if expanded else query
    sources = FOCUS_SOURCES.get(focus, FOCUS_SOURCES["all"])
    per_source = max(1, min(max_results_per_source, 10))
    requested_scrape = DEFAULT_SCRAPE_TOP_N if scrape_top_n is None else scrape_top_n
    scrape_n = max(0, min(requested_scrape, cfg.max_scrape))

    tasks = [_run_source(cfg, src, search_query, per_source) for src in sources]
    results = await asyncio.gather(*tasks)

    all_hits: list[ResearchHit] = []
    errors: list[str] = []
    for hits, err in results:
        all_hits.extend(hits)
        if err:
            errors.append(err)

    ranked = rank_hits(_filter_recency(all_hits, recency), search_query)
    top = ranked[: max(per_source * len(sources), 20)]

    if scrape_n > 0:
        scrape_candidates = [
            h for h in top if h.url and not h.url.endswith(".pdf")
        ][:scrape_n]
        for h in scrape_candidates:
            cached = cache.get("scrape", h.url)
            if cached and cached.get("content"):
                h.scraped_markdown = cached["content"][:4000]
                continue
            scraped = await scrape_url(cfg, h.url)
            if scraped.get("content"):
                cache.set("scrape", h.url, scraped)
                h.scraped_markdown = scraped["content"][:4000]

    summary_parts = [f"Query: {query}"]
    if expanded:
        summary_parts.append(f"Expanded: {', '.join(expanded[1:3])}")
    summary_parts.append(f"Found {len(top)} ranked hits from {len(sources)} source types.")
    if recency != "any":
        summary_parts.append(
            f"Recency: rolling {RECENCY_DAYS[recency]} days in UTC; "
            "unknown and future dates excluded; code/Hub use activity, others publication; "
            "date-only values mean midnight UTC."
        )
    if errors:
        summary_parts.append(f"Partial errors: {len(errors)}")

    resp = ResearchHumanoidResponse(
        query=query,
        expanded_queries=expanded,
        summary=" ".join(summary_parts),
        hits=top,
        errors=errors,
    )
    return resp.model_dump_json(indent=2)
