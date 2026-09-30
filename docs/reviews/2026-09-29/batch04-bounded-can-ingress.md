# Batch 04: frame integrity and bounded CAN ingress

Baseline: `d50d0a63a407a7b1f8f2e01501eb59a2e251cbec`, merged PR215 with
all five required checks successful. Branch: `codex/bounded-can-ingress`.
Decision: [ADR0021](../../decisions/0021-bounded-can-ingress.md).

This dependency-ready slice addresses CS04 malformed receive evidence and the
CAN portion of M06 bounded runtime work. T26 receives meaningful failure-path
coverage; its wider test/tool audit remains incomplete. The local primary,
affected Linux feature and simulator gates pass. Two independent source reviews
find no introduced blocker. All five GitHub jobs and actual Linux virtual CAN
pass at implementation head 6d5cedf; final delivery requires exact-head checks. CS04 and T26 remain partial; M06 is partial because
Pi command work, disk separation and measured jitter remain open.

The public boundaries under test are raw/decoded CAN receive, Supervisor
enable/command admission, and the real virtual-CAN setup script. The unchanged
baseline has four public Supervisor failures: a finite unknown flood is fully
consumed; ignored receive overload permits motion/re-enable; a saturated
pre-enable flush activates; and an Enable-write flood activates instead of
rolling back. Their red log is `batch04/davout-receive-bounds-baseline-red.log`
under the preserved evidence root.

Actual RuntimeBus/SocketCAN short-frame, remote-request and two-port fairness
regressions compile against the unchanged Linux baseline. Temporary draft
[PR216](https://github.com/jaylamping/marengo/pull/216) captured all three specific
behavioral failures on a Linux virtual-CAN kernel at `ba8f116`, run `36678785246`.
Short DLC0 and remote requested DLC0 each produced an extra Status observation;
the first port's statuses preceded the peer fault. Other CI jobs passed. The
candidate now passes all remaining length/type cases; those suffixes are not
claimed as additional original baseline reds. PR216 was
closed without merging after preserving source, logs and a branch bundle. Local
Docker Desktop virtual CAN is unavailable, so compilation and synthetic
envelopes alone do not qualify the concrete SocketCAN conversion.

Review also exposed candidate regressions: grouping motor observations before
transport events reordered Error-before-device and backend-error-before-peer
fault evidence. Both now have public same-time/raw-order failure cases. The
installed SocketCAN classic reader internally uses `read_exact`, which can hide
interruption retries inside a claimed single attempt. The candidate replaces
that path with a safe single-read wrapper around the same classic-mode socket;
SocketCAN FD motor protocol support is not enabled by that wrapper choice.
Pinned SocketCAN 3.5.0 and its socket2 0.5.10 read/write call chains are audited
down to a single nonretrying syscall. Error subscription and nonblocking setup
fail closed; TX uses one write of dependency-owned bytes and rejects short writes.
No manual CAN ABI or workspace unsafe is introduced.

Review found two other converter defects before finalization. An actual typed
three-byte Error frame was expanded to eight bytes; a safe raw classic DLC9
fixture panicked in dependency payload slicing. Separate Linux behavioral reds
precede the guards and actual-DLC slice repair. These are intermediate candidate
failures, not unchanged-baseline regressions.

## Resulting behavior

Transmit encoding remains a fixed eight-byte command. Receive evidence preserves
Data/Remote/Error class, actual classic payload length and RTR requested length.
Configured status types 2/24 and detail type 21 require exactly eight Data bytes.
Malformed traffic retains its available bytes and reason without renewing pose.
Only Data status headers provide device flags/mode. Partial detailed bytes,
warnings and complete fault words remain separate domains. Kernel errors bypass
vendor addressing and retain their entire error mask.

One required nonblocking backend primitive feeds raw, report and compatibility
projections. Every poll has global limits of 64 raw frames and 256 read attempts,
including ignored noise, idle reads and interruptions. Callers can narrow those
limits. Reaching a limit without a full idle pass reports incomplete work; unread
suffixes remain queued. Interfaces are visited one at a time in stable rotating
order, including a bounded peer visit after a backend failure. Positive budgets
are maximum waits with paced idle passes, not hard realtime measurements.

Davout merges device, malformed, kernel and backend evidence by common raw
delivery order even when timestamps tie. Receive overload, malformed configured
feedback or transport failure latches through private fault authority and
attempts every configured stop. Ignoring a returned error cannot permit motion.
Both activation flushes require observed quiescence; post-write failure stops and
rolls back before the enable marker is installed. Diagnostic retention is bounded
to first/latest evidence and counts.

The virtual-CAN setup script now uses a valid short probe name, retains the actual
kernel/root failure and cleans up only a probe it successfully created, including
interruption during creation. Eight real-entrypoint fake-CLI contracts cover
success, unsupported kernel, foreign collision, failed cleanup and signals.

## Regression and test-quality evidence

Three portable public driver regressions fail on the unchanged production
baseline: nine-byte status is accepted, one poll consumes all 257 queued statuses,
and unknown traffic escapes a finite raw quota. All pass the repaired engine.
The four Supervisor reds and three real RuntimeBus reds above also use unchanged
public boundaries. No missing-new-API compile failure is counted as a regression.

Seventeen Supervisor tests include the four baseline regressions and independent
malformed/header/partial-word, raw-order, ignored-error authority and activation
contracts. Twelve new receive conformance tests cover attempts/interruptions,
source-round completion, narrow/oversized/zero limits, retained FIFO suffix,
same-time order, benign idle and legacy prefix/error behavior. Five Linux tests
exercise actual pinned typed conversion. One additional real RuntimeBus test
checks malformed raw bytes, class, actual/requested length and Data-only header
evidence together; it is candidate conformance. The original three RuntimeBus
baseline regression functions remain unchanged and run the entire matrix on the
candidate.

Replaced one old receive test that expected silent partial success and broad
wall-time bounds with an explicit incomplete-error/retained-prefix public
contract. Existing useful positive receive, command, valid-pose and address tests
remain. No blanket test deletion or implementation-mirroring test was added.
Native driver test bodies total about 0.11 seconds, including a deliberate 100ms
benign-idle contract; this is not a cross-environment performance comparison.

## Verification

| Gate | Result and scope |
| --- | --- |
| Native affected owners | Robstride 62 and Davout 133 pass; strict affected Clippy passes. Linux-gated tests are excluded from these counts. |
| Integrated Linux feature | 373 pass, six vCAN tests ignored; includes all five converter tests. Strict affected all-target feature Clippy passes. 11.91 seconds total. |
| Required primary | 630 Rust pass, one ignored; 355 frontend, 72 Pi-MCP, eight vCAN-script and four dependency-helper contracts pass. Format, strict Clippy, protobuf/frontend builds, fresh deny/audit and fatal aarch64 release smoke pass. 101.96 seconds total. |
| Dependency scope | 400 locked crates scanned with mandatory live database/index fetch and strict coverage. Allowed unmaintained paste remains tracked M01; Cargo.lock unchanged. |
| Simulation | Five sim-harness tests and Python engine smoke pass in 2.65 seconds. minimal.xml, nq=2/nv=2; production model and independent plant are not qualified. |
| Independent source review | Two frozen-snapshot reviews find no introduced blocker, including dependency IO, all active adapters, completion/order, authority and final virtual-test wiring. |
| Candidate GitHub/Linux vCAN | All five jobs pass at 6d5cedf, run36684361659. All 73 driver feature tests pass with none ignored, including five converters and six actual virtual tests. CI/Compose use locked dependencies and --include-ignored --test-threads=1. |

Evidence filenames include robstride-existing-api-red.log,
davout-receive-bounds-baseline-red.log, github-baseline-summary.json,
the two davout-*-candidate-red.log ordering receipts,
socketcan-converter-intermediate-red.log,
socketcan-invalid-dlc-intermediate-red.log, vcan-script-red.log,
vcan-script-green.log, affected-linux-gate.log, primary-check.log and
sim-check.log. Review receipts are independent-ingress-review.md and
transport-supervisor-review.md; socketcan-io-source-receipt.md pins the actual
dependency versions and source paths.

All preserved logs, the initial all-ref bundle and baseline archive remain under
`J:/code/marengo-migration-backup-20260929/batch04`. The only active Windows
checkout is `J:/code/marengo`; software and local CAD remain under `J:/code`.
No physical robot operation, limit increase, deployment, or Wave sign-off change
is part of this batch. Firmware byte order/recovery, reference/readback, physical
stop acknowledgement, GPIO, Pi publication and jitter remain explicit work.

Delivery: [PR217](https://github.com/jaylamping/marengo/pull/217), [implementation-head CI](https://github.com/jaylamping/marengo/actions/runs/36684361659). The evidence-only final revision must pass all five exact-head checks before merge. Final merged/head/run identity is retained in batch04/merge-receipt.json under the evidence root and reconciled into the next batch history.
