"""MCP stdio server entrypoint."""

from __future__ import annotations

import asyncio
import json
from typing import Any

from mcp.server import Server
from mcp.server.stdio import stdio_server
from mcp.types import TextContent, Tool

from marengo_research_mcp.cache import ResearchCache
from marengo_research_mcp.config import load_config
from marengo_research_mcp.tools.research import research_humanoid
from marengo_research_mcp.tools.scrape import scrape_url_tool, scrape_urls_tool
from marengo_research_mcp.tools.search import (
    search_forums_tool,
    search_github_tool,
    search_hf_tool,
    search_papers,
    search_reddit_tool,
    search_standards_tool,
    search_vendor_docs_tool,
    search_web_tool,
)

server = Server("marengo-research-mcp")


@server.list_tools()
async def list_tools() -> list[Tool]:
    return [
        Tool(
            name="research_humanoid",
            description="Orchestrator: search papers, code, forums, vendor docs, HF, standards, and web for humanoid robotics topics; optional scrape.",
            inputSchema={
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "focus": {
                        "type": "string",
                        "enum": ["papers", "code", "community", "vendor", "standards", "all"],
                        "default": "all",
                    },
                    "max_results_per_source": {"type": "integer", "default": 5},
                    "scrape_top_n": {"type": "integer", "default": 3},
                    "recency": {
                        "type": "string",
                        "enum": ["year", "month", "week", "any"],
                        "default": "any",
                    },
                },
                "required": ["query"],
            },
        ),
        Tool(
            name="search_papers",
            description="Search arXiv, Semantic Scholar, OpenReview, and Papers With Code.",
            inputSchema={
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "default": 10},
                },
                "required": ["query"],
            },
        ),
        Tool(
            name="search_github",
            description="Search GitHub repositories for humanoid/robotics code.",
            inputSchema={
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "default": 10},
                },
                "required": ["query"],
            },
        ),
        Tool(
            name="search_reddit",
            description="Search Reddit robotics communities.",
            inputSchema={
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "default": 10},
                },
                "required": ["query"],
            },
        ),
        Tool(
            name="search_forums",
            description="Search ROS Discourse and Stack Exchange.",
            inputSchema={
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "default": 10},
                },
                "required": ["query"],
            },
        ),
        Tool(
            name="search_vendor_docs",
            description="Search Robstride/Seeed vendor docs and curated seeds.",
            inputSchema={
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "default": 10},
                },
                "required": ["query"],
            },
        ),
        Tool(
            name="search_hf",
            description="Search Hugging Face models and datasets.",
            inputSchema={
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "default": 10},
                },
                "required": ["query"],
            },
        ),
        Tool(
            name="search_standards",
            description="Search curated ISO/IEC robotics safety standards index.",
            inputSchema={
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "default": 10},
                },
                "required": ["query"],
            },
        ),
        Tool(
            name="search_web",
            description="General DuckDuckGo web search.",
            inputSchema={
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "default": 10},
                },
                "required": ["query"],
            },
        ),
        Tool(
            name="scrape_url",
            description="Scrape a single URL to markdown via trafilatura.",
            inputSchema={
                "type": "object",
                "properties": {"url": {"type": "string"}},
                "required": ["url"],
            },
        ),
        Tool(
            name="scrape_urls",
            description="Batch scrape URLs with concurrency limit.",
            inputSchema={
                "type": "object",
                "properties": {
                    "urls": {"type": "array", "items": {"type": "string"}},
                },
                "required": ["urls"],
            },
        ),
        Tool(
            name="research_status",
            description="Cache stats and server health.",
            inputSchema={"type": "object", "properties": {}},
        ),
    ]


@server.call_tool()
async def call_tool(name: str, arguments: dict[str, Any]) -> list[TextContent]:
    cfg = load_config()
    cache = ResearchCache(cfg)
    args = arguments or {}

    if name == "research_humanoid":
        text = await research_humanoid(
            cfg,
            cache,
            query=args["query"],
            focus=args.get("focus", "all"),
            max_results_per_source=args.get("max_results_per_source", 5),
            scrape_top_n=args.get("scrape_top_n", 3),
            recency=args.get("recency", "any"),
        )
    elif name == "search_papers":
        text = await search_papers(cfg, cache, args["query"], args.get("limit", 10))
    elif name == "search_github":
        text = await search_github_tool(cfg, cache, args["query"], args.get("limit", 10))
    elif name == "search_reddit":
        text = await search_reddit_tool(cfg, cache, args["query"], args.get("limit", 10))
    elif name == "search_forums":
        text = await search_forums_tool(cfg, cache, args["query"], args.get("limit", 10))
    elif name == "search_vendor_docs":
        text = await search_vendor_docs_tool(cfg, cache, args["query"], args.get("limit", 10))
    elif name == "search_hf":
        text = await search_hf_tool(cfg, cache, args["query"], args.get("limit", 10))
    elif name == "search_standards":
        text = await search_standards_tool(args["query"], args.get("limit", 10))
    elif name == "search_web":
        text = await search_web_tool(cfg, cache, args["query"], args.get("limit", 10))
    elif name == "scrape_url":
        text = await scrape_url_tool(cfg, cache, args["url"])
    elif name == "scrape_urls":
        text = await scrape_urls_tool(cfg, cache, args.get("urls", []))
    elif name == "research_status":
        text = json.dumps(
            {
                "ok": True,
                "cache_dir": str(cfg.cache_dir),
                "cache_stats": cache.stats(),
                "github_token_set": bool(cfg.github_token),
            },
            indent=2,
        )
    else:
        text = json.dumps({"error": f"unknown tool: {name}"})

    return [TextContent(type="text", text=text)]


async def async_main() -> None:
    async with stdio_server() as (read_stream, write_stream):
        await server.run(read_stream, write_stream, server.create_initialization_options())


def main() -> None:
    asyncio.run(async_main())


if __name__ == "__main__":
    main()
