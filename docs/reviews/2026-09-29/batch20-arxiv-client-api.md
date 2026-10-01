# Batch20: locked arxiv client API (T18)

Baseline: d03830df9ba6bf6091d47d2ad0eecd71015ef322. Isolated branch `codex/arxiv-client-api`.

The adapter uses `arxiv.Client().results(search)` with the existing query, limit,
sort and normalization. The client retains the locked package defaults for retries
and pagination. Iterator errors propagate to the existing search composition.

An offline stub exposes the locked public API without `Search.results`; two
original tests fail with AttributeError. The identical complete probe passes
following the one-line repair. It checks normalized hits across multiple yielded
results and a failure after the first result. A separate installed-package smoke
checks arxiv 4.0.0 and its actual Search/Client API without making a request.
All nine offline tests pass with warnings fatal on Python 3.12.14.

Review and exact-head Linux primary gate are pending. T18 is not marked verified
before delivery. T17 is a separate branch; its new CI job awaits the user's answer
on GitHub workflow authorization. No network provider or hardware was accessed.
