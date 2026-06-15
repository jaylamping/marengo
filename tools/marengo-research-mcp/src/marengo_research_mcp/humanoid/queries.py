"""Humanoid query expansion."""

from __future__ import annotations

DOMAIN_TERMS = [
    "biped locomotion",
    "whole-body control",
    "model predictive control",
    "MIT actuator control",
    "impedance control",
    "gravity compensation",
    "sim-to-real",
    "URDF humanoid",
    "CAN motor drive",
    "SocketCAN",
    "Robstride",
]

FOCUS_SOURCES = {
    "papers": ["arxiv", "semantic_scholar", "openreview", "papers_with_code"],
    "code": ["github", "huggingface"],
    "community": ["reddit", "forums"],
    "vendor": ["vendor_docs"],
    "standards": ["standards"],
    "all": [
        "arxiv",
        "semantic_scholar",
        "openreview",
        "papers_with_code",
        "github",
        "reddit",
        "forums",
        "vendor_docs",
        "huggingface",
        "standards",
        "web",
    ],
}


def expand_query(query: str) -> list[str]:
    base = query.strip()
    if not base:
        return []
    expanded = [base]
    for term in DOMAIN_TERMS:
        if term.lower() in base.lower():
            continue
        if any(k in base.lower() for k in ("motor", "can", "robstride", "gravity", "impedance")):
            if term in ("MIT actuator control", "CAN motor drive", "Robstride", "gravity compensation"):
                expanded.append(f"{base} {term}")
        elif any(k in base.lower() for k in ("walk", "locomotion", "biped", "humanoid")):
            if term in ("biped locomotion", "whole-body control", "sim-to-real"):
                expanded.append(f"{base} {term}")
    return expanded[:4]
