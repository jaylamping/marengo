"""Shared result types."""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, Field

SourceType = Literal[
    "paper",
    "code",
    "community",
    "forum",
    "vendor",
    "hf",
    "standard",
    "web",
]


class ResearchHit(BaseModel):
    type: SourceType
    title: str
    url: str
    snippet: str = ""
    source_name: str = ""
    year: int | None = None
    authors: list[str] = Field(default_factory=list)
    stars: int | None = None
    citation_count: int | None = None
    pdf_url: str | None = None
    scraped_markdown: str | None = None
    score: float = 0.0


class SearchResponse(BaseModel):
    query: str
    hits: list[ResearchHit] = Field(default_factory=list)
    errors: list[str] = Field(default_factory=list)
    cached: bool = False


class ResearchHumanoidResponse(BaseModel):
    query: str
    expanded_queries: list[str] = Field(default_factory=list)
    summary: str = ""
    hits: list[ResearchHit] = Field(default_factory=list)
    errors: list[str] = Field(default_factory=list)
