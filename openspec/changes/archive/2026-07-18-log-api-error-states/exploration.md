# Exploration: log-api-error-states

Consul log HTTP client collapses all failures to null or empty values. Operators cannot tell whether the UI is showing genuinely empty archive or candump data, or a failure such as missing auth, an unavailable log store, a network problem, or a missing gateway endpoint.

---

## Current State

### log-api.ts

`consul/src/lib/log-api.ts` implements `logFetch`, which returns `T | null`. Public helpers map a null result to empty arrays or objects. HTTP status codes and network failures are indistinguishable from successful responses with no data.

### Hooks

`use-archive-sessions.ts` and `use-candump-data.ts` expose no loading or error state. They use bare `.then()` chains without `.catch()`, which risks unhandled promise rejections if the fetch layer ever throws.

### UI empty states

Components treat empty data as success:

- `LogsSessionList` — empty session list with no error indication
- Candump table — empty rows with no failure context
- `logs-overview` — empty overview treated as normal

`LogsArchiveSearch` is the partial exception: it has loading and error UI, but its error path reflects empty SQLite query results, not HTTP failures from the log API.

### Gateway

The gateway already returns appropriate HTTP statuses (401, 503, 500, 404). No gateway changes are required for this work.

---

## Affected Areas

| File / area | Change |
|-------------|--------|
| `consul/src/lib/log-api.ts` | Discriminated result type and status-to-error mapping |
| `consul/src/hooks/use-archive-sessions.ts` | `{ loading, error, data }` state |
| `consul/src/hooks/use-candump-data.ts` | `{ loading, error, data }` state |
| Session list UI (`LogsSessionList`) | Error banner; mute empty-state only when `!error && empty` |
| `logs-overview` | Same error vs empty distinction |
| `LogsArchiveSearch` | Align HTTP failure handling with new error model |
| Candump UI | Error banner; conditional empty-state |
| `consul/src/lib/log-api.test.ts` (new) | Vitest coverage for status-to-`LogApiError` mapping |

**Out of scope:** `config-api.ts` uses a similar null-collapse pattern but is not part of this change.

---

## Approaches

| Approach | Description | Pros | Cons |
|----------|-------------|------|------|
| **1. Discriminated Result at `logFetch`** | Return `{ ok: true, data: T } \| { ok: false, error: LogApiError }` | Explicit, type-safe, no exceptions in normal flow; hooks compose cleanly | Requires updating all call sites and hook consumers |
| **2. Thrown `LogApiError`** | Throw typed errors from `logFetch`; hooks catch and set state | Familiar async pattern | Easy to miss `.catch()`; mixes control flow with exceptions |
| **3. Envelope error fields on each response** | Add `error` / `status` fields alongside data on every response shape | Backward-compatible surface | Duplicated fields per endpoint; weaker typing at boundaries |

---

## Recommendation

**Approach 1 — discriminated Result at `logFetch`.**

Introduce a `LogApiError` type with kinds:

- `no_endpoint` — gateway log endpoint not configured
- `unauthorized` — 401 (e.g. missing `VITE_MARENGO_LOG_TOKEN`)
- `unavailable` — 503 (log store down)
- `not_found` — 404
- `server` — 5xx
- `network` — fetch failure / unreachable host

Propagate through hooks as `{ loading, error, data }`. UI changes:

- Show a destructive banner when `error` is set
- Show empty-state messaging only when `!error && empty`
- Fix `LogsArchiveSearch` to surface HTTP failures, not only empty SQLite results

Add vitest tests in `log-api.test.ts` for HTTP status → `LogApiError` kind mapping.

**Scope:** Consul-only, approximately 150–250 lines. No gateway or backend changes.

---

## Risks

| Risk | Mitigation |
|------|------------|
| Incomplete call-site migration leaves silent null handling | Grep for `logFetch` and hook consumers; type system forces handling of `ok: false` |
| Empty-state UX regressions (hiding real errors behind “no data”) | Gate empty-state on `!error && empty` in each affected component |
| `LogsArchiveSearch` conflates SQLite empty with HTTP error | Separate error sources in that component when wiring the new model |
| Unhandled rejections if any path still throws | Keep `logFetch` non-throwing; hooks use Result branches only |

---

## Ready for Proposal

**Yes.**

Problem, current behavior, affected files, recommended approach, and scope are defined. Gateway behavior is confirmed sufficient. Next step: `/sdd-propose log-api-error-states` to draft the change proposal and acceptance criteria.
