# Batch27 — await cold research searches and gate offline tool tests

Baseline: `ca13eb810406539cc2394d6130881176fb76665c`. Branch:
`codex/research-cache-await-windows`. Findings: T17; related T26 gate coverage.

All six public cached search handlers wrap async providers in ordinary lambdas.
The prior coroutine-function check therefore calls those lambdas without awaiting
the returned coroutine, then passes the coroutine to response validation. Cold
requests raise a Pydantic list error before a response or cache entry exists.
The shared helper now explicitly accepts an async provider and awaits its result.
Provider and response-validation errors remain structured response errors under
the existing cache policy. Invalid persisted responses refresh from their source.
Versioned cache envelopes independently qualify finite timestamps, future dates
and the configured TTL; legacy/unrecognized envelopes are misses.

The frozen public-handler probe exercises every affected handler with literal
results and real provider failures, real temporary disk cache miss/hit, preserved
timestamps/metadata, exact provider arguments and a separate cache key for a
changed limit. External source calls are the only substituted boundary.
The original v1 probe's12 failures replayed unchanged green, but Spec review found
its undeclared metadata field was discarded by Pydantic. V1 is preserved as
preliminary await/error/cache evidence; its metadata claim is excluded. Canonical
v2 uses declared authors/year/stars and literal timestamp/field expectations.
All12 v2 cases fail at the actual archived baseline and replay unchanged green.
The qualification receipt binds its complete bytes and source identities.
Twelve additional public malformed-provider/disk-response cases and12 independent
cache-envelope/TTL cases qualify the review corrections. Before repair,21 of those
24 fail and3 boundary/expiry positives pass. A final Spec review found invalid
UTF-8 bytes also escaped cache reads. One public-handler regression fails at
17d3385 with UnicodeDecodeError, then passes unchanged after treating corrupt
bytes as a cache miss. Final Windows and Pi offline suites each pass83 tests.
The initial import-error preparation is preserved/excluded from behavior evidence.
This repairs the
missing T17 source change independently of the Mac-only unpublished branch;
its local commits/workflow are preserved and no workflow-token scope is changed.

The required check script now runs the locked offline research Python suite and
daily-audit parser/scanner-runner contracts. Docker supplies uv0.9.27; native
cloud setup installs the same release. A disposable project environment avoids
reusing or destroying a Windows/Mac `.venv` in the bind-mounted checkout.
Network-marked research tests remain excluded. T26 stays partial: Compound,
other Python/shell suites and complete tooling dependency auditing still remain.

## Delivery reconciliation

PR240/batch26 is already merged at the selected baseline. Exact PR-head CI
36937530222 passed its applicable check/vcan jobs (sim skipped by path policy).
Exact merged-main CI36938244248 passed allfive jobs. T22 can therefore be
verified; the old handoff's draft/pending text is historical. Counts before this
batch's delivery:24 verified,11 partial,67 open,102 IDs and8 maintenance tasks.

## Physical-host validation scope

Owner authorized Pi connectivity and physical-host validation, and requires an
explicit prompt/confirmation before every movement test. No response within ten
minutes means leave that test pending and continue independent work; silence
never authorizes movement. Arm is reported resting at gravity home. E-stop
readiness and current reference under repaired admission remain unqualified.

SSH to `joey@marengo.local` succeeds. Installed revision is
`4bc77ba605834fdec04b436daa4bec67bca84fbb`, predating all review repairs.
Pi/gateway/CAN services are active, gateway health passes, NTP is synchronized,
both physical CAN interfaces are UP/ERROR-ACTIVE at1Mbit/s, and allfive right-arm
joint observations are present. Decoded protobuf safety reports Disabled.
Legacy Verified homing fields do not establish the repaired private-reference
contract. No enable, motion, CAN transmit request, restart, install or deployment
was issued. Device observations do not certify direction, stop, support or model
acceptance.

Initial research source and probe were separately staged under
`/home/joey/marengo-validation/batch27-20261001`, with isolated uv/environment/cache.
Initial Pi replay12 reds/unchanged12 greens/full58 passed. Final review-corrected
replay is isolated beneath its `review-corrected` directory, restoring both
exact original cache/search files for the baseline control.
The portable replay script performs original-red then repaired-green and the
full offline suite without writing installed runtime/configuration or opening
CAN. The initial required Linux check passed813Rust/1existingignored, Consul/PiMCP,
research58/daily-audit15 and a fatal ARM release build. Final corrected primary/Pi
replay passed (canonical12 red/unchanged12 green/full82), followed by the
final83-test Pi run after the UTF-8 correction. Final required Linux check passed813 Rust/1 existing ignored, Consul361,
Pi MCP72, research83, daily-audit15 and the fatal ARM release build.
Final source/primary receipts bind the qualification; independent final review
and hosted delivery remain pending.
The isolated Pi Rust workspace run failed in cs24_constructor_period: its
positive simulation control unexpectedly transmitted diagnostics. A targeted
rerun reproduced the failure. Inspection found the shared config resolver chose
installed /opt/marengo/config rather than the copied diagnostics-off fixture.
This is a separate test-resource isolation finding, pending repair/qualification;
full Pi Rust acceptance is not claimed. No physical CAN transport was opened.
T17 is verified after PR241 merged asadef857. Exact PR CI36956898047
passed applicable changes/build/check/vcan jobs (sim skipped by path policy),
and exact merged-main CI36958210262 passed allfive. Both reviews are clear
at4778422. Final count25 verified,11partial,66open; T26 remains partial.
Batch28/PR242 separately repairs the now-qualified Pi fixture isolation defect.
