"""Dedupe and rank research hits."""

from __future__ import annotations

from urllib.parse import urlparse

from marengo_research_mcp.models import ResearchHit

SOURCE_WEIGHT = {
    "vendor_seed": 1.3,
    "vendor/ddg": 1.1,
    "semantic_scholar": 1.2,
    "arxiv": 1.15,
    "openreview": 1.1,
    "papers_with_code": 1.05,
    "github": 1.0,
    "standards_index": 1.0,
}


def _norm_url(url: str) -> str:
    p = urlparse(url.lower().rstrip("/"))
    return f"{p.netloc}{p.path}"


def dedupe_hits(hits: list[ResearchHit]) -> list[ResearchHit]:
    seen: set[str] = set()
    out: list[ResearchHit] = []
    for h in hits:
        key = _norm_url(h.url) if h.url else h.title.lower()
        if key in seen:
            continue
        seen.add(key)
        out.append(h)
    return out


def score_hit(hit: ResearchHit, query: str) -> float:
    q = query.lower()
    score = SOURCE_WEIGHT.get(hit.source_name, 0.9)
    title = hit.title.lower()
    snippet = hit.snippet.lower()
    for tok in q.split():
        if tok in title:
            score += 0.15
        if tok in snippet:
            score += 0.05
    if hit.citation_count:
        score += min(hit.citation_count / 500.0, 0.5)
    if hit.stars:
        score += min(hit.stars / 5000.0, 0.5)
    if hit.year and hit.year >= 2023:
        score += 0.1
    return score


def rank_hits(hits: list[ResearchHit], query: str) -> list[ResearchHit]:
    deduped = dedupe_hits(hits)
    for h in deduped:
        h.score = score_hit(h, query)
    return sorted(deduped, key=lambda x: x.score, reverse=True)
