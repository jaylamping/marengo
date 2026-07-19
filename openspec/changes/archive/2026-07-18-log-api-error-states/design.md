# Design: Log API Error States

## Technical Approach

Introduce a `LogApiResult<T>` discriminated union in `consul/src/lib/log-api.ts`. `logFetch` becomes non-throwing: it maps the unset Chappe URL, HTTP status, and thrown/parse failures to a `LogApiError` kind, and every exported `fetch*` threads `LogApiResult` to callers (satisfies spec *LogApiResult public surface* + *Error taxonomy*). Hooks (`use-archive-sessions`, `use-candump-data`) hold **named per-resource slices** so an errored fetch never blanks an unrelated resource. UI reads each slice's `error` first, renders a destructive banner, and gates empty-state on `!error && empty` (spec *Per-resource hook async state* + *UI error banners*).

## Architecture Decisions

| Decision | Choice | Rejected | Rationale |
|----------|--------|----------|-----------|
| Failure channel | Discriminated `LogApiResult<T>` at `logFetch` | Thrown typed errors; per-response envelope fields | Type system forces `ok` handling; keeps API non-throwing (no missed `.catch`); matches exploration Approach 1 |
| Hook state shape | Explicit named slices (`sessionsState`, `linesState`, `pageState`, `summaryState`), each `{ loading, error, data }` | Single shared slice per hook; generic keyed reducer | Spec mandates independent errors; flat named fields keep reducers readable and typed |
| Banner rendering | Local inline `text-destructive` markup + small shared helper | New shadcn `Alert` component | No `Alert` exists today; `logs-connection-banner` / `logs-archive-search` already use inline destructive text — follow existing convention |
| `no_endpoint` UX | Quiet (empty/disabled), no banner | Same destructive banner as other kinds | Demo/offline mode has no endpoint by design; a banner would be noise |
| Search line-fetch guard | Skip `fetchBenchLines`/`fetchTraceLines` when `archiveView === 'search'` | Fetch then discard | Avoids spurious errors and wasted requests on the search tab |

## Data Flow

```
fetch(path) ──try/catch──▶ logFetch ──▶ LogApiResult<T>
   │ throw/bad JSON → {ok:false, network}      │
   │ !res.ok → status map (below)              ▼
   └───────────────────────────────▶ exported fetch* (no ?? [] collapse)
                                               │
                                               ▼
                              hook slice { loading, error, data }
                                               │
                        ┌──────────────────────┴───────────────────────┐
                        ▼                                               ▼
                error set → <banner>                    !error && empty → empty copy
                                               │
                                       else → data view
```

### Status → kind map (ordered)

| Condition | kind |
|-----------|------|
| Chappe URL unset (no fetch) | `no_endpoint` |
| 401 | `unauthorized` |
| 404 | `not_found` |
| 503 | `unavailable` |
| other 5xx | `server` |
| other `!res.ok` | `server` |
| fetch throws / invalid JSON | `network` |

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `consul/src/lib/log-api.ts` | Modify | Add `LogApiResult`/`LogApiError`/`LogErrorKind`; try/catch + status map in `logFetch`; all `fetch*` return `Promise<LogApiResult<…>>`; drop `?? []`/`?? {}`; inline blob-404 fidelity comment |
| `consul/src/lib/log-api.test.ts` | Create | Six kind mappings + thrown fetch + invalid JSON → `network` |
| `consul/src/hooks/use-archive-sessions.ts` | Modify | Named slices for sessions vs archive lines; `archiveView==='search'` line-fetch guard; branch on `result.ok` |
| `consul/src/hooks/use-candump-data.ts` | Modify | Named slices for candump page vs summary |
| `consul/src/components/dashboard/logs/logs-session-list.tsx` | Modify | Accept `error` prop; destructive banner; empty copy only when `!error && empty` |
| `consul/src/components/dashboard/logs/logs-overview.tsx` | Modify | Pass slice errors into session list / archive panel / candump; banner + gated empty |
| `consul/src/components/dashboard/logs/logs-archive-search.tsx` | Modify | Branch `fetchStructuredLogs` on `ok`; HTTP error ≠ "No structured logs in SQLite yet" |
| `consul/src/components/dashboard/logs/candump-frame-table.tsx` | Modify | Accept `error`; banner; gate "No frames" empty-state |
| `consul/src/components/dashboard/logs/__tests__/logs-glass-shell.test.tsx` | Modify | `fetchStructuredLogs` mock returns `LogApiResult`; add HTTP-fail-vs-empty assertion |

## Interfaces / Contracts

```ts
export type LogErrorKind =
  | 'no_endpoint' | 'unauthorized' | 'not_found'
  | 'unavailable' | 'server' | 'network';

export type LogApiError = { kind: LogErrorKind; status?: number };

export type LogApiResult<T> =
  | { ok: true; data: T }
  | { ok: false; error: LogApiError };
```

Hook slice: `type AsyncSlice<T> = { loading: boolean; error: LogApiError | null; data: T }`.

## Testing Strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit | `logFetch` kind mapping | Vitest, mock `fetch`/`res` for 401/404/503/500/502, unset URL, throw, bad JSON |
| Unit | `fetch*` non-collapse | Assert `ok:false` on failure — no empty substitute |
| Component | HTTP fail ≠ empty | `LogsSessionList` error → banner, no empty copy; `{ok:true, []}` → empty copy, no banner |
| Component | Archive search | `unavailable` → HTTP error, not SQLite-empty message |
| Regression | `logs-glass-shell` | Mocks use `LogApiResult`; suite passes with `npm test`/`npm run build` |

## Non-Goals

- Gateway / `config-api.ts` changes — gateway HTTP statuses already correct.
- Live hydrate error UI for `useArchiveSessions` (`fetchRecentLogs`) — deferred.
- i18n of error copy; motor/safety/control paths.

## Migration / Rollout

No migration. Consul-only TypeScript; revert `log-api.ts` to restore prior behavior. No backend/proto state touched.

## Open Questions

None blocking.
