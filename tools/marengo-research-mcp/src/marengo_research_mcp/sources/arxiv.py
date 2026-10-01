"""arXiv search via arxiv Python package."""

from __future__ import annotations

from datetime import datetime

import arxiv

from marengo_research_mcp.dates import provider_datetime
from marengo_research_mcp.models import ResearchHit


def search_arxiv(query: str, limit: int = 10, *, window: tuple[datetime, datetime] | None = None) -> list[ResearchHit]:
    hits: list[ResearchHit] = []
    dated_query = f"cat:cs.RO AND ({query})"
    if window is not None:
        start, end = window
        # Provider minute precision is a superset; local filtering stays exact.
        dated_query += f" AND submittedDate:[{start:%Y%m%d%H%M} TO {end:%Y%m%d%H%M}]"
    search = arxiv.Search(
        query=dated_query,
        max_results=limit,
        sort_by=arxiv.SortCriterion.SubmittedDate,
    )
    for paper in arxiv.Client().results(search):
        year = paper.published.year if paper.published else None
        hits.append(
            ResearchHit(
                type="paper",
                title=paper.title.replace("\n", " ").strip(),
                url=paper.entry_id,
                snippet=(paper.summary or "")[:500],
                source_name="arxiv",
                year=year,
                published_at=provider_datetime(paper.published),
                updated_at=provider_datetime(getattr(paper, "updated", None)),
                authors=[a.name for a in paper.authors[:5]],
                pdf_url=paper.pdf_url,
            )
        )
    return hits
