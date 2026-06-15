"""Curated ISO/IEC standards index."""

from __future__ import annotations

from marengo_research_mcp.models import ResearchHit

STANDARDS = [
    {
        "title": "ISO 13482 — Personal care robots",
        "url": "https://www.iso.org/standard/53820.html",
        "snippet": "Safety requirements for personal care robots including mobile servant and physical assistant robots.",
        "tags": ["safety", "humanoid", "personal care", "iso"],
    },
    {
        "title": "ISO 10218 — Industrial robots safety",
        "url": "https://www.iso.org/standard/51330.html",
        "snippet": "Robot systems and integration safety requirements; relevant for industrial humanoid arms.",
        "tags": ["safety", "robot", "iso"],
    },
    {
        "title": "IEC 62443 — Industrial automation security",
        "url": "https://webstore.iec.ch/en/publication/7033",
        "snippet": "Cybersecurity for industrial automation and control systems including networked robots.",
        "tags": ["security", "network", "iec"],
    },
    {
        "title": "ISO/TS 15066 — Collaborative robots (force limiting)",
        "url": "https://www.iso.org/standard/62996.html",
        "snippet": "Safety requirements for collaborative industrial robot systems and workspaces.",
        "tags": ["safety", "collaborative", "force"],
    },
]


def search_standards(query: str, limit: int = 10) -> list[ResearchHit]:
    q = query.lower()
    hits: list[ResearchHit] = []
    for entry in STANDARDS:
        blob = f"{entry['title']} {entry['snippet']} {' '.join(entry['tags'])}".lower()
        if any(tok in blob for tok in q.split()) or any(
            tag in q for tag in entry["tags"]
        ):
            hits.append(
                ResearchHit(
                    type="standard",
                    title=entry["title"],
                    url=entry["url"],
                    snippet=entry["snippet"],
                    source_name="standards_index",
                )
            )
    if not hits:
        for entry in STANDARDS[:limit]:
            hits.append(
                ResearchHit(
                    type="standard",
                    title=entry["title"],
                    url=entry["url"],
                    snippet=entry["snippet"],
                    source_name="standards_index",
                )
            )
    return hits[:limit]
