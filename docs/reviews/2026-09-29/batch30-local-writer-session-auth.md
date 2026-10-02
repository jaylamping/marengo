# Batch30: authenticated and bounded local limits mirror

Baseline: `1e2141f13d8c9bfa01188068df714b605ab0077e`. Qualified source:
`a0ec58e8530903eb44b3eddb02b882a7390b42c5`, branch
`codex/local-writer-session-auth`, worktree
`J:/code/marengo-worktrees/local-writer-session-auth`.

The actual listener now requires an approved Origin, JSON type and runtime
session credential before parsing or writer admission. It bounds body/output,
request and worker time, session work rate, and concurrent operations. The
asynchronous child is terminated and reaped after a deadline/output failure.
Consul takes the local credential in Hardware and retains it only in tab memory;
the mirror runs after durable Pi acknowledgement. The local CLI accepts negative
numeric arguments and binds YAML writes to the supplied checkout's config tree,
matching its configured URDF. Ordinary runtime resolver precedence is unchanged.

Frozen v1 proves three original refused-request failures plus a valid positive
control; all four replay unchanged green. Actual real-CLI v3 tests reproduce
negative argument rejection and competing config redirection, then replay
unchanged green in all26 cases on Linux and Windows. Frozen v4 additionally
reproduces zero-width hard bounds reaching a writer; all27 unchanged cases pass
on Linux, Windows and native aarch64 Pi. The tests qualify pre-writer refusal,
stalled body/worker deadlines, responsive HTTP, process reaping, recovery,
output/exit failure, rate limits, credential rotation and exact hard/soft values
in disposable YAML/URDF copies. See `evidence/batch30/source-qualification.json`.

The primary gate passed at the qualified source:814 Rust tests/1 ignored,
364 Consul,72 Pi MCP,27 local writer,83 research,15 daily audit,7 disposable
installer tests, plus fmt/clippy, required dependency scans and fatal aarch64
cross-build. With a known LIMIT_SYNC_TOKEN fixture present during the production
build, all99 emitted files exclude that marker. This qualifies the local mirror
credential only; broader gateway/Auto Learn build-token work remains G07.

Pi tests run from the user-owned validation checkout, with official Node24.16.0
verified by its release SHA256 and extracted locally. Six touched input hashes
match the candidate, including generated server JS. The installed revision is
still4bc77ba; marengo-pi/gateway/can remain active. Native full-workspace
qualification and exact-final hosted CI/delivery are recorded separately.
No installation, privileged operation, service restart, physical CAN command or
motor movement occurred. Gravity home does not qualify support, current
reference or E-stop readiness. Movement requires a new explicit confirmation
and commissioning checks; ten-minute silence only permits independent work.

## Standards

Independent review of the qualified source:0 documented violations and0
actionable smell findings. Explicit resources, runtime credentials, bounded
work and isolated child environments follow repository guidance.

## Spec

Independent review:0 scoped implementation defects;1 partial full-acceptance
requirement. T04 asks that a valid mirror update the exact accepted generation.
This batch mirrors accepted values; generation/transaction identity remains T03.
T04 therefore stays partial. Multi-file power-loss durability and physical
acceptance are not established. The report's earlier stale pending-test list
has been replaced with these actual receipts.

Totals: Standards0; Spec1 partial acceptance (T03 generation prerequisite).
All102 finding IDs,8 maintenance tasks, existing work and historical evidence
remain intact. Automation stays PAUSED and safety limits/Wave sign-off unchanged.

Preparation exclusion: the first fresh-worktree UI collection lacked ignored
protobuf generation; generation and actual tests/build then passed. It is neither
behavioral red nor green qualification. Archived v3 evidence predates v4's added
zero-width case; each retained version is immutable.
