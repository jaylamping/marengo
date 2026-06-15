"""Headless CLI for daily audit research appendix."""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
from pathlib import Path

from marengo_research_mcp.cache import ResearchCache
from marengo_research_mcp.config import load_config
from marengo_research_mcp.tools.research import research_humanoid


async def audit_research(topics_file: Path, output: Path | None) -> int:
    cfg = load_config()
    cache = ResearchCache(cfg)
    data = json.loads(topics_file.read_text(encoding="utf-8"))
    topics = data.get("topics", [])
    sections: list[str] = ["# Industry research appendix\n"]
    for topic in topics:
        q = topic if isinstance(topic, str) else topic.get("query", "")
        if not q:
            continue
        focus = topic.get("focus", "all") if isinstance(topic, dict) else "all"
        result = await research_humanoid(
            cfg,
            cache,
            query=q,
            focus=focus,
            max_results_per_source=3,
            scrape_top_n=0,
        )
        parsed = json.loads(result)
        sections.append(f"## {q}\n")
        sections.append(parsed.get("summary", ""))
        sections.append("")
        for hit in parsed.get("hits", [])[:5]:
            sections.append(f"- [{hit.get('title')}]({hit.get('url')}) ({hit.get('source_name')})")
        sections.append("")
    md = "\n".join(sections)
    if output:
        output.write_text(md, encoding="utf-8")
    else:
        sys.stdout.write(md)
    return 0


def main() -> None:
    parser = argparse.ArgumentParser(description="Marengo research MCP CLI")
    sub = parser.add_subparsers(dest="cmd", required=True)
    audit = sub.add_parser("audit-research", help="Run research for daily audit topics file")
    audit.add_argument("--topics-file", type=Path, required=True)
    audit.add_argument("-o", "--output", type=Path, default=None)
    args = parser.parse_args()
    if args.cmd == "audit-research":
        raise SystemExit(asyncio.run(audit_research(args.topics_file, args.output)))


if __name__ == "__main__":
    main()
