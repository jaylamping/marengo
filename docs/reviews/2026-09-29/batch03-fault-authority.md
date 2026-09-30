# Batch03: fault evidence, stop outcomes and scanner coverage

Baseline: main `e830bc5952ddc587186eec36bba08f5ffc01147a` (merged PR214).
Windows source, local CAD and recovery evidence remain under `J:\code`.
This batch is a software repair; no physical robot was contacted or operated.
Review and delivery: [PR215](https://github.com/jaylamping/marengo/pull/215).

## Observable contracts

Robstride now supplies every addressed status/fault observation in delivery
order, including the prefix preceding a terminal receive error. The six status
flags, drive mode, four detailed-fault bytes and four warning bytes remain
separate domains. Fault-only traffic never creates or refreshes a pose. Raw
bytes remain available without inventing a detailed-field endian convention or
claiming installed firmware qualification. Compatibility maps remain lossy;
Davout consumes the lossless report.

Davout retains runtime faults outside its pose cache and exposes a read-only
`SafetySnapshot`. Fault identity and first cause survive healthy feedback,
Disable, synthetic seeding and cache clearing. Motion, enable and calibration
routes refuse while latched. Reserved mode cannot authorize a pose; unexpected
current-session Reset/Calibration on an enabled address faults. An inactive
peer's Reset status does not stop a scoped active joint. Warning-only reports
remain diagnostic. Invalid operator requests are rejected without a runtime
fault or stop burst.

Every raw device/mode/hard-position hazard is inspected before pose coalescing.
Position-derived velocity is evaluated once per address per drain. Consecutive
host dequeue times cannot reconstruct sample spacing in a queued burst. Equal
or older timestamps never renew freshness, but cannot hide hazard evidence.

Stop attempts zero speed, neutral MIT and ordinary Disable for every configured
address despite earlier failures. A failed write makes the result fail and is
retained in the stop report. The first failed report survives later successful
stops, and delivery errors do not replace the initiating fault. Software
Disabled and accepted transport writes do not prove physical stop. Ordinary
Disable contains no firmware fault-clear flag. The software E-stop input hook
now latches and attempts stop; releasing it cannot grant recovery. GPIO wiring
is still absent.

Berthier propagates both post-send receive failures and fallible planner-entry
refresh. Planner and torque intent consult the persistent latch. Davout's stop
generation cancels old planner, Wave and torque intent even when Disable and a
new Enable happen between ticks. Missing-feedback bootstrap exhaustion and
ascent stalls latch a controller fault. This is the bounded slice in
[ADR0020](../../decisions/0020-lossless-feedback-and-fault-authority.md), within
the remaining ownership migration of ADR0019.

## Dependency gate and TLS maintenance

Docker and cloud pin cargo-deny 0.20.2 and cargo-audit 0.22.2. The gate requires a
current advisory/index fetch, then denies `index-failure`; scanner errors are
fatal locally and in CI. The obsolete October 2025 database pin is removed.
Real missing-index, unreachable-database and failed warm-cache refresh fixtures
prove incomplete coverage cannot pass. Four inexpensive CLI propagation checks
also guard the helper's failure sequencing.

The current scan exposed unmaintained rustls-pemfile 2.2.0
([RUSTSEC-2025-0134](https://rustsec.org/advisories/RUSTSEC-2025-0134.html)).
Gateway uses maintained rustls-pki-types PEM parsing and axum-server 0.8, whose
[release notes](https://docs.rs/crate/axum-server/0.8.0/source/CHANGELOG.md)
describe that replacement. Cargo.lock changes only axum-server and removes
rustls-pemfile. No advisory ignore is added. Existing allowed `paste` maintenance
work remains M01; TLS lifecycle/rotation remains G12.

## Behavior and review evidence

Baseline tests exercise existing public interfaces; compilation failure on the
baseline is never counted as red evidence. Ten Berthier regressions fail on the
unchanged baseline, including hidden post-send RX/unsafe-pose errors, mode entry
installing intent after failure, silent-bootstrap rearm, old torque replay and
new intent after an observed fault. Three existing-driver-API regressions catch
discarded status flags, lost high detailed bytes and lost terminal-error prefix.
New report/snapshot interfaces additionally have direct contract tests.

Independent review caught an introduced false speed fault for twelve valid
queued poses: the identical public Supervisor/MemoryBus probe passed baseline,
failed the intermediate candidate, then passed after coalescing. Review also
caught request-limit errors being incorrectly latched, equal-time unsafe pose
being skipped, and inactive peer Reset being incorrectly treated as active.
Each correction has an assertion failure and final green evidence.

Two low-value tests are replaced: a private truncated little-endian `u16` fault
assertion becomes asymmetric full-domain/ordering tests; hashing the same DER
twice becomes certificate-chain, matching-private-key and malformed material
contracts through `load_or_generate_tls`. Existing controller replays retain
their planner/stall assertions. This does not claim the entire legacy test
audit is finished.

Exact logs, unchanged baseline archive, review receipts, scanner provenance and
probe sources are under `J:\code\marengo-migration-backup-20260929\batch03`.
Integrated primary, simulation, virtual CAN and GitHub results are recorded in
the implementation ledger after execution; merge requires all applicable gates.

Final local primary passes 598 Rust tests (one ignored), 355 frontend and 72 Pi
MCP tests, strict lint/build, current deny/audit and aarch64 release. The scan
has zero index failures against database `f23b768236fe2880e4cfa167da662cad8ca79240`.
Cargo.lock SHA256 is `52FB481D34BBCDBA9EFBD5A594B3494D193DB301ED2ED85CD91C43BDDCC40CCB`;
unrelated QUIC dependency-edge changes from the initial Cargo update are reverted
and locked metadata resolution succeeds. Simulation has five passing tests and
a minimal engine smoke. Docker Desktop kernel
`6.6.87.2-microsoft-standard-WSL2+` rejects even a valid short-name virtual CAN
probe; both actual Linux virtual CAN tests pass on GitHub, including the new
lossless report test in zero/positive budget modes. All five code checks pass
at `3feeab4` in run `36674876363`; the final evidence revision receives all
applicable checks before merge. The first
primary attempt was deliberately interrupted before its test-tree freeze, and
its separate log is not counted as a completed check.

## Remaining work

CS08 and CS12 are verified software repairs. Across the 101 finding IDs, the
ledger now records 12 verified, 7 partial and 82 open; overlapping IDs do not
count independent defects. The software implementation and legacy test-quality
work remain incomplete, so the repair heartbeat continues.

CS04 remains partial: malformed DLC handling, installed firmware identity/field
qualification and explicit fresh recovery are pending. CS13 remains partial:
Pi/protobuf persistent fault publication, other runtime-owner failure classes,
session/generation command admission and recovery remain pending. T26 retains
missing Python/shell/Compound suites and tool dependency audits. M06 retains
bounded continuous receive work and actual Pi jitter measurement. Public raw
bus/config/synthetic APIs remain trusted bypasses under CS15.

Physical acquisition correlation, drive-local torque/timeout readback, motor
acknowledgements, E-stop wiring/support, current reference/model acceptance and
the unfinished commissioning ladder/Wave work require separate operator
acceptance. No caps or Wave sign-off change, and no physical acceptance is
inferred from these checks.
