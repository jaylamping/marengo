# log-api-error-model Specification

## Purpose

Consul log/archive/candump fetches MUST distinguish empty data from auth, network, store, or endpoint failures.

## Requirements

### Requirement: LogApiResult public surface

Every exported `fetch*` MUST return `Promise<LogApiResult<T>>`. Helpers MUST NOT return `T | null` or collapse `{ ok: false }` with `?? []` / `?? {}`.

#### Scenario: Failure does not masquerade as empty

- GIVEN a non-success HTTP response or fetch failure
- WHEN any exported `fetch*` runs
- THEN result is `{ ok: false, error: LogApiError }` with no empty substitute

### Requirement: Error taxonomy and status mapping

`LogApiError.kind` MUST be one of: `no_endpoint`, `unauthorized`, `not_found`, `unavailable`, `server`, `network`. Mapping order: unset Chappe URL → `no_endpoint` (no fetch); 401 → `unauthorized`; 404 → `not_found`; 503 → `unavailable`; other 5xx → `server`; other !ok → `server`; fetch throw or invalid JSON → `network`.

#### Scenario: Ordered HTTP mapping

- GIVEN mocked 401, 404, 503, 500, and 502 responses
- WHEN `logFetch` runs for each
- THEN each returns the kind per the ordered table

#### Scenario: Network and parse are non-throwing

- GIVEN `fetch` throws or body is invalid JSON
- WHEN `logFetch` runs
- THEN promise resolves `{ ok: false, error: { kind: 'network' } }`

### Requirement: Per-resource hook async state

`useArchiveSessions` MUST keep separate async slices for sessions vs archive lines (bench/trace). `useCandumpData` MUST keep separate slices for candump page vs summary. Each slice updates only from its own fetch.

#### Scenario: Independent resource errors

- GIVEN sessions fetch fails and bench lines would succeed
- WHEN archive mode shows bench tab with selected session
- THEN sessions slice errors without blocking bench lines slice

### Requirement: UI error banners and empty-state gating

`LogsSessionList`, logs overview archive panel, and candump UI MUST show a destructive banner when the relevant slice has an error. Empty-state copy MUST render only when `!error && empty`. `no_endpoint` MUST render quietly (disabled/empty), not as a banner. `unauthorized` copy MUST say the gateway token was rejected or required, not Vite env messaging.

#### Scenario: HTTP fail vs genuine empty

- GIVEN sessions fetch returns server error vs `{ ok: true, sessions: [] }`
- WHEN session list renders
- THEN error shows banner without empty copy; success-with-empty shows empty copy without banner

#### Scenario: no_endpoint is quiet

- GIVEN all log fetches return `no_endpoint`
- WHEN logs UI renders offline
- THEN no destructive error banner appears

### Requirement: Archive search HTTP vs SQLite empty

`LogsArchiveSearch` MUST surface `fetchStructuredLogs` HTTP failures separately from zero-match SQLite results. SQLite-empty messaging MUST appear only after `{ ok: true }` with zero entries.

#### Scenario: HTTP failure is not SQLite empty

- GIVEN structured log fetch returns `{ ok: false, error: { kind: 'unavailable' } }`
- WHEN user runs search
- THEN HTTP error is shown, not "No structured logs in SQLite yet"

### Requirement: Search view must not fetch archive lines

When `archiveView === 'search'`, the archive hook MUST NOT call `fetchBenchLines` or `fetchTraceLines`.

#### Scenario: Search tab skips line fetch

- GIVEN archive mode, selected session, `archiveView === 'search'`
- WHEN hook effects run
- THEN neither bench nor trace line fetch is invoked

### Requirement: Gateway fidelity documentation

Log API source MUST document that blob endpoints may map store errors to HTTP 404, making `not_found` lossy. No gateway changes in this work.

#### Scenario: Blob 404 fidelity documented

- GIVEN log API module source
- WHEN reviewing blob endpoint errors
- THEN inline comment explains possible 404 conflation

### Requirement: Automated verification

`consul/src/lib/log-api.test.ts` MUST cover all six kinds plus thrown fetch and invalid JSON → `network`. At least one hook/component test MUST assert HTTP failure ≠ empty sessions and ≠ SQLite empty. `logs-glass-shell.test.tsx` mocks MUST use `LogApiResult`.

#### Scenario: Test suite passes

- GIVEN updated unit and component tests
- WHEN `cd consul && npm test` runs
- THEN kind-mapping and regression tests pass
