# Batch27 — await cold research searches and gate offline tool tests

Baseline: `ca13eb810406539cc2394d6130881176fb76665c`. Branch:
`codex/research-cache-await-windows`. Findings: T17; related T26 gate coverage.

All six public cached search handlers wrap async providers in ordinary lambdas.
The prior coroutine-function check therefore calls those lambdas without awaiting
the returned coroutine, then passes the coroutine to response validation. Cold
requests raise a Pydantic list error before a response or cache entry exists.
The shared helper now explicitly accepts an async provider and awaits its result.
Provider errors remain structured response errors under the existing cache policy.

The frozen public-handler probe exercises every affected handler with literal
results and real provider failures, real temporary disk cache miss/hit, preserved
timestamps/metadata, exact provider arguments and a separate cache key for a
changed limit. External source calls are the only substituted boundary.
At the actual baseline all12 cases fail with the unawaited-coroutine validation
error; unchanged complete test bytes pass all12 after repair. Whole-probe SHA256:
`f2a174407b0afd5c72e0b140ceda9e000d067a2457b709b74df968783421fd84`.
The full offline research suite passes58 on native Windows. This repairs the
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

Current research source and the unchanged probe are separately staged under
`/home/joey/marengo-validation/batch27-20261001`, with isolated uv/environment/cache.
The portable replay script performs original-red then repaired-green and the
full offline suite without writing installed runtime/configuration or opening
CAN. Pi replay, primary check, independent review and delivery evidence will be
recorded after completion. T17 remains open until required delivery passes.
