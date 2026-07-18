# Proposal: Log API Error States

## Intent

Consul log/archive/candump fetches collapse all failures to `null` or empty values.
Operators cannot distinguish "no data" from "failed to fetch" — auth failures, a downed
log store, missing gateway endpoints, and network/parse errors are silently swallowed.
Every exported `fetch*` helper is affected, not only the private `logFetch` primitive.

## Scope

### In Scope

- Introduce `LogApiResult<T>` discriminated union (`{ ok: true, data: T } | { ok: false, error: LogApiError }`) in `log-api.ts`
- Define `LogApiError` kind enum: `no_endpoint | unauthorized | not_found | unavailable | server | network`; status→kind order: 401→unauthorized, 404→not_found, 503→unavailable, other 5xx→server, other !ok→server, throw/parse→network
- Wrap `logFetch` `fetch` + `res.json()` in try/catch so network and parse failures become `{ ok: false, error: { kind: 'network' } }` — API stays non-throwing
- All exported `fetch*` helpers in `log-api.ts` return `Promise<LogApiResult<…>>`; ban `?? []` / `?? {}` collapse on `ok: false`
- **Per-resource async state** in both hooks — NOT a single `{ loading, error, data }` per hook:
  - `useArchiveSessions`: separate state for sessions list vs archive lines (bench/trace)
  - `useCandumpData`: separate state for candump page vs summary
  - Each empty-state gated on that resource's `error`, not a shared field
- Add error banners to `LogsSessionList`, `logs-overview`, and candump UI; gate empty-state on `!error && empty`
- Fix `LogsArchiveSearch` false empty-state (HTTP failure ≠ SQLite empty)
- Guard: `archiveView === 'search'` must not trigger archive line fetch
- `no_endpoint` kind renders quietly (disabled/empty), not a screaming banner — important in demo/offline mode
- Unauthorized copy: "token rejected or required by gateway," not Vite env messaging
- Document gateway fidelity limit in code: blob endpoints may map store errors to 404; kinds for those paths may be lossy — **no gateway changes in this change**
- Vitest: update `logs-glass-shell.test.tsx` mocks; new `log-api.test.ts` with full kind coverage

### Out of Scope

- `config-api.ts` — same pattern, deferred
- Gateway changes — gateway already returns correct HTTP statuses
- Motor/safety/control paths — Consul only
- Live hydrate for `useArchiveSessions` — deferred
- i18n of error strings

## Capabilities

### New Capabilities

- `log-api-error-model`: Discriminated result type, error kind taxonomy, and per-resource async state for all Consul log API fetches

### Modified Capabilities

None — no existing `openspec/specs/` entries yet.

## Approach

1. `logFetch`: wrap `fetch` + `res.json()` in try/catch; map status codes → `LogApiError` kind per ordered table; return `LogApiResult<T>` — never throw.
2. All exported `fetch*`: thread `LogApiResult` through; remove `?? []` / `?? {}` at call sites.
3. Hooks: replace single `{ loading, error, data }` with keyed per-resource state; each resource updates independently.
4. UI: read per-resource `error` first; banner on fail; suppress empty-state while error is set; `no_endpoint` → quiet/disabled.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `consul/src/lib/log-api.ts` | Modified | Result type, try/catch, status map, all exported helpers |
| `consul/src/lib/log-api.test.ts` | New | Kind coverage + thrown fetch + invalid JSON tests |
| `consul/src/hooks/use-archive-sessions.ts` | Modified | Per-resource state (sessions, lines) |
| `consul/src/hooks/use-candump-data.ts` | Modified | Per-resource state (page, summary) |
| `consul/src/components/LogsSessionList` | Modified | Error banner; guarded empty-state |
| `consul/src/components/logs-overview` | Modified | Error banner; guarded empty-state; search guard |
| `consul/src/components/LogsArchiveSearch` | Modified | HTTP vs SQLite error fix |
| Candump UI component(s) | Modified | Error banner; guarded empty-state |
| `consul/src/**/__tests__/logs-glass-shell.test.tsx` | Modified | Update mocks for new Result shape |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Missed exported helper leaves silent null path | Low | TypeScript enforces `ok` branch; grep all `fetch*` exports |
| Per-resource state adds hook complexity | Med | Keep reducers flat; one state slice per fetch site |
| Gateway 404 from store error looks like `not_found` | Med | Document fidelity limit inline; no fix in this change |
| `no_endpoint` banner in demo/offline breaks UX | Low | Render quiet/disabled state, not error banner |

## Rollback Plan

All changes are Consul-only TypeScript. Revert `log-api.ts` to restore `T | null`; hooks and UI revert to empty-state-only behavior. No backend state is touched. `git revert` or branch drop is sufficient.

## Dependencies

None — gateway already returns correct HTTP statuses; no backend or proto changes required.

## Success Criteria

- [ ] All exported `fetch*` in `log-api.ts` return `Promise<LogApiResult<…>>`; `T | null` removed from public API surface; no `?? []` / `?? {}` collapse on `ok: false`
- [ ] `logFetch` try/catch: thrown `fetch` and invalid JSON → `{ ok: false, error: { kind: 'network' } }` — never throws
- [ ] Both hooks use per-resource state; each empty-state gated on that resource's `error`
- [ ] Error banners render in `LogsSessionList`, `logs-overview`, and candump UI on API failure; suppressed when no error
- [ ] `LogsArchiveSearch` surfaces HTTP errors, not false SQLite-empty errors
- [ ] `archiveView === 'search'` does not trigger archive line fetch
- [ ] `no_endpoint` renders quietly (no error banner) in demo/offline mode
- [ ] `log-api.test.ts` covers all six kind mappings + thrown fetch + invalid JSON → network
- [ ] At least one hook/component regression test: HTTP fail ≠ empty sessions banner ≠ empty SQLite
- [ ] `logs-glass-shell.test.tsx` mocks updated; `npm test` passes; `npm run build` produces no TS errors
