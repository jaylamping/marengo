---
name: marengo-research-mcp
description: Humanoid robotics research MCP — prefer research_humanoid first; cite URLs; use vendor docs for motor/CAN questions.
---

# Marengo Research MCP

Use when the user asks about humanoid robotics research, papers, open-source stacks, vendor motor docs, or industry standards.

## Workflow

1. Call **`research_humanoid`** with the user's question (default entry point).
2. If the question is source-specific, call granular tools directly:
   - Motors/CAN/Robstride → `search_vendor_docs`
   - Papers → `search_papers`
   - Code/repos → `search_github`
   - Community → `search_reddit` or `search_forums`
   - Policies/datasets → `search_hf`
   - Safety standards → `search_standards`
3. Use **`scrape_url`** only when snippets are insufficient and URL is not already scraped by orchestrator.
4. Always cite URLs in responses. Large payloads live in `.marengo-research/` cache.

## Daily audit

When running daily standards review, read [docs/daily-audit-rubric.md](../../docs/daily-audit-rubric.md) and prefer vendor docs + [docs/safety.md](../../docs/safety.md) over paper-only motor advice.

## Do not

- Use Firecrawl or generic web search when this MCP is available.
- Scrape PDFs when abstract/metadata from paper search is enough.
