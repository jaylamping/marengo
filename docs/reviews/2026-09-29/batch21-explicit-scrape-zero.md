# Batch21: honor explicit zero scraping (T19)

Baseline d03830df9ba6bf6091d47d2ad0eecd71015ef322; isolated branch
`codex/explicit-scrape-zero`. Review and required exact-head gate are pending.

A shared default of three aligns the MCP schema, dispatcher and Python function.
Omission or None selects that default; explicit zero selects no scraping. Negative
counts clamp to zero and positive counts cap at configured max_scrape. The CLI's
explicit zero now suppresses scraping as intended. Search/ranking behavior is unchanged.

The frozen offline public-orchestrator probe checks omission, None, zero, positive,
negative and over-cap requests with fresh file caches and injected provider/scraper
boundaries. Four original assertion failures plus three passing controls become
seven unchanged positives. Schema/function default alignment is included. All13
offline tests pass with warnings fatal; no network requests or hardware access.
Evidence/qualification.json preserves the complete probe SHA-256 and outcomes.

T19 remains unverified until independent review, required gate and delivery.
