"""arXiv search via arxiv Python package."""

from __future__ import annotations

import arxiv

from marengo_research_mcp.models import ResearchHit


def search_arxiv(query: str, limit: int = 10) -> list[ResearchHit]:
    hits: list[ResearchHit] = []
    search = arxiv.Search(
        query=f"cat:cs.RO AND ({query})",
        max_results=limit,
        sort_by=arxiv.SortCriterion.SubmittedDate,
    )
    for paper in search.results():
        year = paper.published.year if paper.published else None
        hits.append(
            ResearchHit(
                type="paper",
                title=paper.title.replace("\n", " ").strip(),
                url=paper.entry_id,
                snippet=(paper.summary or "")[:500],
                source_name="arxiv",
                year=year,
                authors=[a.name for a in paper.authors[:5]],
                pdf_url=paper.pdf_url,
            )
        )
    return hits
