"""Papers With Code API search."""

from __future__ import annotations

import httpx

from marengo_research_mcp.config import Config
from marengo_research_mcp.models import ResearchHit

PWC_API = "https://paperswithcode.com/api/v1/papers/"


async def search_papers_with_code(cfg: Config, query: str, limit: int = 10) -> list[ResearchHit]:
    params = {"search": query, "page_size": limit}
    headers = {"User-Agent": cfg.user_agent}
    hits: list[ResearchHit] = []
    try:
        async with httpx.AsyncClient(timeout=30.0, headers=headers) as client:
            resp = await client.get(PWC_API, params=params)
            if resp.status_code != 200:
                return hits
            data = resp.json()
    except httpx.HTTPError:
        return hits

    for paper in data.get("results", []):
        url = paper.get("url_abs") or paper.get("url_pdf") or ""
        if not url:
            continue
        hits.append(
            ResearchHit(
                type="paper",
                title=paper.get("title", ""),
                url=url,
                snippet=(paper.get("abstract") or "")[:500],
                source_name="papers_with_code",
                pdf_url=paper.get("url_pdf"),
            )
        )
    return hits
