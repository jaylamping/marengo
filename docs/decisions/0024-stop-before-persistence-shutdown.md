# ADR 0024: stop before persistence on graceful owner shutdown

Status: accepted for software implementation, September 30, 2026.

## Context

On checked PR219 merge `eae4fc3`, the Pi waits for config persistence before
attempting motor stop. Its shared shutdown flag also terminates the writer with
accepted queued work, and stdin Quit continues the current dispatch/tick. These
are source findings awaiting actual behavioral baseline proof. Storage cannot
be an emergency-stop prerequisite. Existing drive support and `disable_on_exit`
policy must remain explicit; accepted stop writes do not confirm physical stop.

## Decision

The installed runtime exits command dispatch when shutdown is observed. Its
graceful exit composition inhibits controller intent, attempts Davout's existing
all-address stop when the configured exit policy requests it, retains its exact
error/StopReport, and only then initiates a bounded persistence drain. No later
motion tick or command dispatch occurs after that exit lifecycle begins.
The no-disable policy reports an explicit skipped stop rather than success.
Do not change master policy, limits, support requirements or Wave sign-off.

Owner shutdown and writer termination are separate lifecycle events. Close new
queue admission at drain start; accepted retained/in-flight writes continue to
their matching success/failure result independently of the owner shutdown flag.
Drain reports timeout/unfinished/worker failure honestly. Disk success or a later
successful stop cannot erase the initiating stop uncertainty. Queue-idle must
not precede completion publication. Existing latest-draft coalescing and broader
configuration CAS/client outcome deficiencies remain explicitly scoped; do not
claim complete ConfigAuthority or new reference durability from this repair.

Keep runtime composition thin and controller intent/stop behavior in its owning
library where useful. Extract existing lifecycle/worker behavior without fixing
it first when needed for deterministic tests; review and freeze that extraction
before reproducing the failure. Use actual ControlLoop/Davout and actual worker
filesystem writes with gates at the I/O/wait seam. Do not duplicate main into a
test-only implementation or count source-order/missing-API checks as regressions.

## Verification and limits

Require a gated writer trace proving every original stop address/action was
attempted before storage waiting, including failed stop; pending accepted work
must drain or remain explicitly unfinished. Prove Quit/shutdown dispatch cannot
reach later queued commands/ticks. Use event handshakes and bounded cleanup,
not five-second negative sleeps. Preserve and replay unchanged probes, distinguish
parity extraction from original-binary proof, then run strict affected checks,
the required primary gate and exact-head/equal-tree main CI.

CS09 may close only for its verified software contract. CS07/CS13/T05/T12,
priority/flood scheduling M06, installed-owner client/proto cutover, reference
transactions, broader configuration persistence and physical stop/timeout/support
acceptance remain separate work. This loop never operates or deploys to the robot.
