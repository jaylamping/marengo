"""Semantic Scholar API search."""

from __future__ import annotations

import httpx

from marengo_research_mcp.config import Config
from marengo_research_mcp.models import ResearchHit

S2_URL = "https://api.semanticscholar.org/graph/v1/paper/search"


async def search_semantic_scholar(
    cfg: Config, query: str, limit: int = 10
) -> list[ResearchHit]:
    params = {
        "query": query,
        "limit": limit,
        "fields": "title,url,abstract,year,authors,citationCount,externalIds,openAccessPdf",
    }
    headers = {"User-Agent": cfg.user_agent}
    async with httpx.AsyncClient(timeout=30.0, headers=headers) as client:
        resp = await client.get(S2_URL, params=params)
        resp.raise_for_status()
        data = resp.json()

    hits: list[ResearchHit] = []
    for paper in data.get("data", []):
        pdf = paper.get("openAccessPdf") or {}
        ext = paper.get("externalIds") or {}
        arxiv_id = ext.get("ArXiv")
        url = paper.get("url") or (f"https://arxiv.org/abs/{arxiv_id}" if arxiv_id else "")
        if not url:
            continue
        authors = [a.get("name", "") for a in paper.get("authors", [])[:5]]
        hits.append(
            ResearchHit(
                type="paper",
                title=paper.get("title", ""),
                url=url,
                snippet=(paper.get("abstract") or "")[:500],
                source_name="semantic_scholar",
                year=paper.get("year"),
                authors=authors,
                citation_count=paper.get("citationCount"),
                pdf_url=pdf.get("url"),
            )
        )
    return hits
