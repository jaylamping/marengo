# Batch31: shared gateway access and runtime credentials

G07 is software verified at dc88d28323a859f7d74d5c50dc8f5689646142d4 against
main a5c4cdb6448c5a0164ebbeffb1c82eb09ab0c266. PR245 is the delivery candidate;
the final documentation head and merged main still require all five successful
exact-head checks before delivery is complete. ADR0033 records the contract.

The configured gateway credential now covers legacy commands, mutations and
sensitive subscriptions before body parsing, publication, file changes or
management work. One immutable startup policy is shared by HTTP, HTTPS and
WebTransport, with explicit Control, Calibration, Configuration, Management and
SensitiveRead capabilities. Legacy LOG and OPERATOR retain administrator
compatibility; narrower configured roles stay narrow. Browser Origins are
restricted, with authenticated non-browser clients permitted without Origin.
Existing attestation, confirmation, joint, rate, runtime freshness and Davout
checks remain additional requirements. This does not repair G06 lifecycle gates.

WebTransport uses a16KiB/five-second subscription bound, a typed bounded admission
response and64 owned application session tasks. Unauthorized mixed subscriptions
create no receiver. Consul reports connected only after admission; credential
changes retire old callbacks and runtime facts. Underlying transport-library
HTTP/3 negotiation is not qualified as a separate connection-queue repair.

Consul provides capability-specific or operator runtime entry and a separate
Auto Learn entry. Values remain in tab memory. API clients and subscriptions use
those values; build/deployment paths no longer read or fetch Pi credentials into
Vite. The qualified build injects11 disposable credential fixtures and scans all
99 emitted files, and is part of CI and deployment build paths.

## Evidence and validation

The frozen original-public HTTP v2 has11 actual refusal failures and2 positive
controls; all13 cases replay unchanged green. An actual original production build
materializes both gateway and Auto Learn markers, and the real asset oracle
rejects it. Candidate conformance covers29 routes with four credential/Origin
cases,8 refused sensitive streams,25 role cases,2 body limits and tuning role
controls. Real loopback HTTP/HTTPS and pinned QUIC qualify admission, delivery,
oversized/absent/partial frames, timeout cleanup and recovery.

Final primary v3 passes826 Rust tests with1 existing ignored humanoid geometry
test,374 Consul,72 Pi MCP,27 local writer,83 research,15 daily audit and7
disposable installer tests, plus fmt/clippy/buf/dependency checks and fatal ARM
release. Native Pi passes the same826 Rust/1 existing ignored and374 Consul,
generation/checksum, build and asset checks. All1736 staged files are bound to
the reviewed commit;364 Rust/build/config/proto/URDF inputs were unchanged from
the first native snapshot. Compilation ran at reduced priority with one worker;
frontend tests ran with one worker and bounded Node heap.

Independent Standards and Spec reviews are clear at the final source. The one
initial duplicate receive-loop heuristic was resolved with a shared consumer.
Source CI36975018460 passes changes, build-image, check, sim and vcan. The frozen
probes are unchanged. See [qualification](evidence/batch31/final-source-qualification.json),
[reviews](evidence/batch31/independent-reviews.md), original receipts and full logs.
Compilation/fixture preparations and the stale generated checksum are explicitly
excluded from behavioral-red evidence.

## Device and continuation

The installed Pi remains4bc77ba605834fdec04b436daa4bec67bca84fbb. Its gateway
responds and reports Disabled; marengo-pi, gateway and CAN services remain active.
No install, sudo, restart, physical CAN operation or motor movement occurred.
Native qualification uses user-owned staging, disposable files, isolated Bus and
loopback listeners. Software tests do not establish deployed or physical acceptance.

The owner reports gravity home. Support, repaired current reference, E-stop and
recovery remain to be qualified before deployment/motor testing. Prompt before
every movement, including right-arm direction checks, with bounds and stop steps;
require explicit confirmation and commissioning checks. Ten-minute silence leaves
movement pending and permits independent work only. Safety limits, Wave sign-off,
paused automations, CAD and existing work remain intact.

The ledger preserves102 IDs and8 maintenance tasks:26 verified,13 partial,63 open.
Next dependency work is WP04 R2b1 durable current-reference journal, still without
motor permission, followed by current selected grant; T01/T02 versioned activation
and T03 exact-generation persistence remain separate. T04 remains partial for T03.
