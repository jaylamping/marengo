"""Environment-based configuration."""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path


def _env(name: str, default: str) -> str:
    return os.environ.get(name, default)


def _env_int(name: str, default: int) -> int:
    raw = os.environ.get(name)
    if raw is None:
        return default
    return int(raw)


@dataclass(frozen=True)
class Config:
    cache_dir: Path
    github_token: str | None
    reddit_client_id: str | None
    reddit_client_secret: str | None
    max_scrape: int
    scrape_timeout_s: float
    cache_ttl_hours: int
    max_concurrent_scrape: int
    user_agent: str


def load_config() -> Config:
    workspace = Path(_env("MARENGO_WORKSPACE", os.getcwd()))
    cache = Path(_env("MARENGO_RESEARCH_CACHE_DIR", str(workspace / ".marengo-research")))
    return Config(
        cache_dir=cache,
        github_token=os.environ.get("GITHUB_TOKEN"),
        reddit_client_id=os.environ.get("REDDIT_CLIENT_ID"),
        reddit_client_secret=os.environ.get("REDDIT_CLIENT_SECRET"),
        max_scrape=_env_int("MARENGO_RESEARCH_MAX_SCRAPE", 5),
        scrape_timeout_s=float(_env("MARENGO_RESEARCH_SCRAPE_TIMEOUT_S", "30")),
        cache_ttl_hours=_env_int("MARENGO_RESEARCH_CACHE_TTL_HOURS", 24),
        max_concurrent_scrape=_env_int("MARENGO_RESEARCH_MAX_CONCURRENT_SCRAPE", 4),
        user_agent=_env(
            "MARENGO_RESEARCH_USER_AGENT",
            "Mozilla/5.0 (compatible; MarengoResearchMCP/0.1; +https://github.com/jaylamping/marengo)",
        ),
    )
