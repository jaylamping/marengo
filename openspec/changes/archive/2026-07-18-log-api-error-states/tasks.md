# Tasks: Log API Error States

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~320–380 |
| 400-line budget risk | Medium |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | single-pr |
| Chain strategy | pending |

Decision needed before apply: Yes
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Medium

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Full change | PR 1 | log-api + hooks + UI + vitest; Consul-only |

## Phase 1: Foundation (log-api types + logFetch)

- [x] 1.1 Add `LogErrorKind`, `LogApiError`, `LogApiResult<T>` exports to `consul/src/lib/log-api.ts`
- [x] 1.2 Create `consul/src/lib/log-api.test.ts` — RED: mock `fetch` for 401/404/503/500/502 → kinds per ordered table
- [x] 1.3 RED: unset Chappe URL → `{ ok: false, kind: 'no_endpoint' }`; thrown fetch + invalid JSON → `network`
- [x] 1.4 GREEN: refactor `logFetch` — try/catch on fetch+json; status→kind map; return `LogApiResult<T>`; add blob-404 fidelity comment

## Phase 2: Core (fetch* exports + hooks)

- [x] 2.1 Update all 7 exported `fetch*` in `log-api.ts` to return `Promise<LogApiResult<…>>`; remove `?? []`/`?? {}` collapse
- [x] 2.2 Extend `log-api.test.ts` — each `fetch*` returns `{ ok: false }` on failure, not empty substitute
- [x] 2.3 Refactor `consul/src/hooks/use-archive-sessions.ts` — `sessionsState` + `linesState` slices (`AsyncSlice<T>`); branch on `result.ok`
- [x] 2.4 Add search guard in `use-archive-sessions.ts` — skip `fetchBenchLines`/`fetchTraceLines` when `archiveView === 'search'`
- [x] 2.5 Refactor `consul/src/hooks/use-candump-data.ts` — `pageState` + `summaryState` slices; branch on `result.ok`

## Phase 3: UI wiring (banners + empty-state gating)

- [x] 3.1 `logs-session-list.tsx` — accept `error` prop; destructive banner; empty copy only when `!error && empty`; skip banner for `no_endpoint`
- [x] 3.2 `logs-overview.tsx` — pass slice errors to session list, archive panel, candump; wire per-slice loading/error/data
- [x] 3.3 `logs-archive-search.tsx` — branch `fetchStructuredLogs` on `ok`; HTTP error ≠ SQLite-empty; `unauthorized` copy per spec
- [x] 3.4 `candump-frame-table.tsx` — accept `error`; banner; gate "No frames" on `!error && empty`; quiet `no_endpoint`

## Phase 4: Tests + verification

- [x] 4.1 Component test: `LogsSessionList` — server error → banner, no empty copy; `{ ok: true, [] }` → empty, no banner
- [x] 4.2 Component test: `LogsArchiveSearch` — `unavailable` → HTTP error, not "No structured logs in SQLite yet"
- [x] 4.3 Update `__tests__/logs-glass-shell.test.tsx` mocks to `LogApiResult`; add HTTP-fail-vs-empty assertion
- [x] 4.4 Run `cd consul && npm test && npm run build` — all pass, no TS errors
