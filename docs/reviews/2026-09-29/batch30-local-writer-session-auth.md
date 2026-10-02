# Batch30 — authenticated and bounded local limits mirror

Baseline: `1e2141f13d8c9bfa01188068df714b605ab0077e`. Worktree:
`J:/code/marengo-worktrees/local-writer-session-auth`, branch
`codex/local-writer-session-auth`. This is WP10 T04 work in progress, not a
delivery or closure. All102 finding IDs and8 maintenance tasks remain intact.

The actual loopback server accepted foreign-origin text/plain requests and
missing/wrong credentials. The frozen public v1 fixture replaces only the writer
with a noop that records invocation. Three negative cases fail against original
source and its valid positive control passes. All four cases replay unchanged
green against the candidate; no repository config or physical device is written.

The candidate rejects unapproved/missing origins, requires a per-session Bearer
credential and JSON type, validates scalar bounds, limits body/output and
request time, runs the writer asynchronously, admits one worker at a time and
caps authenticated session requests. Runtime credentials are generated at boot
or supplied by trusted LIMIT_SYNC_TOKEN. The Consul Hardware sheet accepts the
credential at runtime and keeps it only in tab memory until reload. Local sync
uses it after durable Pi acceptance and stops before fetch when absent.

Initial public auth tests4 pass; targeted Consul tests15 and production build
pass. The first fresh-worktree hardware suite could not collect because ignored
generated protobuf was absent. This is preparation failure, excluded from green
or behavioral red counts; generation and actual tests/build subsequently pass.

Remaining: malformed and oversized payload/output, request/worker deadlines,
concurrency/rate tests, actual mirror argument fidelity, required-gate wiring,
build-token absence evidence, independent review, full primary and delivery.
T03 generation/authority work is not claimed fixed. No Pi installation, service
restart, physical CAN operation or motor movement occurs.
