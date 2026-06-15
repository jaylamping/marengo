"""trafilatura + httpx page extraction."""

from __future__ import annotations

import asyncio

import httpx
import trafilatura
from markdownify import markdownify

from marengo_research_mcp.config import Config

MAX_MARKDOWN_CHARS = 12_000


async def fetch_html(cfg: Config, url: str) -> str:
    headers = {"User-Agent": cfg.user_agent}
    async with httpx.AsyncClient(
        timeout=cfg.scrape_timeout_s, headers=headers, follow_redirects=True
    ) as client:
        resp = await client.get(url)
        resp.raise_for_status()
        return resp.text


def extract_markdown(html: str, url: str) -> tuple[str, str]:
    text = trafilatura.extract(html, url=url, include_links=True, output_format="markdown")
    if text and len(text) >= 200:
        return text[:MAX_MARKDOWN_CHARS], "trafilatura"
    fallback = markdownify(html)
    cleaned = fallback.strip()[:MAX_MARKDOWN_CHARS]
    if len(cleaned) >= 200:
        return cleaned, "markdownify"
    return cleaned, "failed"


async def scrape_url(cfg: Config, url: str) -> dict[str, str]:
    try:
        html = await fetch_html(cfg, url)
        content, source = extract_markdown(html, url)
        return {"url": url, "content": content, "source": source}
    except Exception as exc:
        return {"url": url, "content": "", "source": "failed", "error": str(exc)}


async def scrape_urls(cfg: Config, urls: list[str]) -> list[dict[str, str]]:
    sem = asyncio.Semaphore(cfg.max_concurrent_scrape)

    async def one(u: str) -> dict[str, str]:
        async with sem:
            return await scrape_url(cfg, u)

    return list(await asyncio.gather(*[one(u) for u in urls]))
