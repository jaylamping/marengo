# Marengo Research MCP

Python MCP server for humanoid robotics research: papers, code, forums, vendor docs, Hugging Face, standards, and general web. Also runs headless for the daily audit.

`GITHUB_TOKEN` is optional but helps with rate limits. The launcher (`run-mcp.ps1` / `run-mcp.sh`) pulls it from `gh auth token` when you are logged in via GitHub CLI.

## Setup

1. Install [uv](https://docs.astral.sh/uv/) (or use `python -m venv` + pip)
2. From repo root:

```bash
just research-mcp-setup
```

3. Restart the marengo-research MCP server in Cursor.

## Tools

| Tool | Description |
|------|-------------|
| `research_humanoid` | Orchestrator: queries all sources, ranks, optional scrape |
| `search_papers` | arXiv, Semantic Scholar, OpenReview, Papers With Code |
| `search_github` | GitHub repos |
| `search_reddit` | Reddit + DDG fallback |
| `search_forums` | ROS Discourse + Stack Exchange |
| `search_vendor_docs` | Robstride/Seeed seeds + web |
| `search_hf` | Hugging Face models/datasets |
| `search_standards` | ISO/IEC curated index |
| `search_web` | DuckDuckGo |
| `scrape_url` / `scrape_urls` | trafilatura + httpx |
| `research_status` | Cache stats |

## Cache

Results land in `.marengo-research/` (gitignored).

## Headless CLI (daily audit)

```bash
cd tools/marengo-research-mcp
uv run python -m marengo_research_mcp.cli audit-research \
  --topics-file ../../var/log/daily-audit/YYYY-MM-DD/topics.json \
  -o ../../var/log/daily-audit/YYYY-MM-DD/research.md
```

## Tests

```bash
cd tools/marengo-research-mcp
uv sync --extra dev
uv run pytest tests/ -q
```

## Daily audit

[docs/daily-audit.md](../../docs/daily-audit.md), [docs/daily-audit-rubric.md](../../docs/daily-audit-rubric.md).
