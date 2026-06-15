"""Curated vendor documentation search."""

from __future__ import annotations

from pathlib import Path

import yaml

from marengo_research_mcp.config import Config
from marengo_research_mcp.models import ResearchHit
from marengo_research_mcp.sources.web import search_duckduckgo

_SEEDS_PATH = Path(__file__).resolve().parent.parent / "humanoid" / "seeds.yaml"


def _load_vendor_seeds() -> list[dict[str, str]]:
    if not _SEEDS_PATH.exists():
        return []
    data = yaml.safe_load(_SEEDS_PATH.read_text(encoding="utf-8")) or {}
    return list(data.get("vendor_docs", []))


async def search_vendor_docs(cfg: Config, query: str, limit: int = 10) -> list[ResearchHit]:
    hits: list[ResearchHit] = []
    q_lower = query.lower()
    for entry in _load_vendor_seeds():
        title = entry.get("title", "")
        url = entry.get("url", "")
        tags = entry.get("tags", [])
        blob = f"{title} {' '.join(tags)}".lower()
        if not url:
            continue
        if any(tok in blob for tok in q_lower.split()) or any(
            t in q_lower for t in tags
        ):
            hits.append(
                ResearchHit(
                    type="vendor",
                    title=title,
                    url=url,
                    snippet=entry.get("description", "")[:500],
                    source_name="vendor_seed",
                )
            )

    if len(hits) < limit:
        ddg = await search_duckduckgo(
            cfg,
            f"robstride OR seeed robstride {query}",
            limit=limit - len(hits),
        )
        for h in ddg:
            h.type = "vendor"
            h.source_name = "vendor/ddg"
            hits.append(h)
    return hits[:limit]
