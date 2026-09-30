# Feedback, command and declarative-policy admission

Batch baseline: main `9215db0b70cca1de365d38154731cc8775dcb579`
(merged PR213). Branch: `codex/feedback-command-validity`. All recording buses,
scratch baselines, logs and CAD remain under `J:\code`. No robot was contacted.
Delivery: [PR214](https://github.com/jaylamping/marengo/pull/214), code commit
`d4c869bb489862a865b08a6a0d5af6f507f28fb5`.

## Result and scope

CS01 and CS03 have verified software regressions, independent review, a passing
primary gate and all five GitHub code checks. The final documentation head is
rechecked before merge.
CS04, CS14 and CS15 remain partial. This batch establishes command admission and
receive-time freshness; persistent fault recovery, complete stop outcomes,
immutable config generations and physical safety acceptance remain separate work.

- Every active motor needs its own finite pose received after the current enable
  marker and within the watchdog deadline. Empty drains, unknown/inactive traffic,
  fault-only frames and older/equal replayed timestamps cannot refresh pose.
  Invalid feedback blocks motion until a strictly newer valid pose arrives.
- Enable drains queued status before its writes and again before committing the
  Active marker. A failed final drain enters the existing disable cleanup path.
  Berthier tracks that marker even when disable/re-enable occurs between ticks.
  It sends at most two neutral startup ticks before `MissingFeedback`; Davout
  independently enforces the elapsed bootstrap deadline. Neutral means zero
  kp/kd, desired velocity and feedforward. Gravity/PD is skipped until fresh pose.
- Davout validates all fields and configured identities in a MIT batch, performs
  checked joint-to-motor conversion, and prepares every command before the first
  transmit. Failed admission restores limiter/watchdog command history. Valid
  bus delivery can still fail partway through; this is not atomic CAN delivery.
- Robstride's real encoder/send paths reject nonfinite values, negative gains,
  duplicate addresses, wrong register kinds and invalid routes before sending.
  Firmware target floats must be finite; gain and speed/torque caps must also be
  nonnegative. Both generic firmware write paths reject unsupported byte modes;
  the typed helper preserves supported modes 0–3. The unused standard-ID
  compatibility encoder was removed after a repository-wide consumer search.
- Shared config admission covers finite numbers, timing budgets, unique addresses
  and joints, signed direction/positive gearing, complete active mapping/type
  agreement, hard/soft bounds, gains, friction, limit margins and effective
  homing settings. Loaders, profile writers, Supervisor startup and candidate
  overlays use this policy. Overlay rejection precedes live mutation/enqueue.
  Berthier gain setters reject malformed or unknown-joint batches before state
  mutation; their callers now handle errors.

## Independent regressions and evidence

Evidence root: `J:\code\marengo-migration-backup-20260929\batch02`. Tests use the
actual public interfaces. Wire expectations are literal vendor fields or
independent numeric endpoints, rather than encoder/decoder round trips.

| Regression | Observed red before the corresponding repair | Green contract/evidence |
|---|---|---|
| Davout freshness/command admission | All 14 initial public regressions fail against the unchanged baseline, including silence masked by drains, silent peers, old poses, invalid transforms/fields and valid-prefix emission | `crates/davout/tests/command_admission.rs`; `davout-followup-red.log`, final `davout-suite.log` |
| Status queued during enable writes | A status placed in real MemoryBus RX by the Enable write later authorizes nonneutral output; one motion frame escapes | Additional public admission test and unchanged independent scratch probe; `davout-enable-race-red.log`, `queue-race-red.log`, `queue-race-green.log` |
| Final enable drain failure | Enable succeeds when the final receive should fail | Additional public test requires an error, Disabled, no enable marker and a recorded Disable write; `davout-enable-race-red.log` |
| Driver NaN torque/invalid batches | Valid first frame and NaN second frame are emitted; raw RS03 torque word 0 denotes the -60 Nm wire endpoint | `crates/robstride/tests/command_validity.rs`; `robstride-baseline-red.log`, `robstride-green.log` |
| Firmware register admission | Baseline admits seven wrong-kind/negative writes | Independent old-source public probe, `parameter-baseline-red.log`; new kind and unsigned payload checks in driver contracts |
| Unsupported firmware modes | Modes 4, 5 and 255 are emitted through both generic bus APIs | All byte values 4–255 reject with no output; independent golden values 0–3 remain accepted; `runmode-baseline-red.log`, `runmode-evidence.md` |
| Declarative config | Seven tests report 52 malformed policy cases admitted by the original loaders/validator | `crates/marengo-config/tests/safety_validation.rs`; `config-red.log`, `config-suite.log`; separate missing-torque-cap and ki-over-cap red logs |
| Config overlay rollback | Negative friction `fv` is accepted through the actual overlay | Negative `fv`/`k`, persistent and transient cases all fail before live/disk mutation; `overlay-red.log`, `caller-green.log` |
| Runtime gains | NaN replaces a valid override; malformed multi-joint batches alter existing valid state | Public controller tests preserve state for all four gain fields and unknown/invalid joints; `gain-red.log`, `caller-green.log` |
| Missing-pose startup | Original controller emits raw FF 33095 instead of neutral 32767 before any current-session pose | Public controller wire test, `bootstrap-red.log`, `caller-green.log` |
| Re-enable between ticks | With neutral startup fixed but mode-only detection retained, new enable returns `MissingFeedback` on its first tick | Marker-based bounded window; `bootstrap-reenable-red.log`, `caller-green.log` |
| Taught range excludes zero | Controller startup sends zero target into valid elbow range [0.2,0.8], returning Limit instead of soliciting pose | Positive and negative taught ranges emit only neutral in-range targets, then expire; `bootstrap-bounds-red.log`, `controller-contracts-green.log` |

The first bootstrap experiment accidentally reused an old-source Cargo output
from the shared target volume. A compile error is not red regression evidence.
Only the five affected package build artifacts were cleaned before testing the
candidate; subsequent runs rebuild its sources. Future archived-baseline runs
must use a separate `CARGO_TARGET_DIR`. Native baseline Berthier also encounters
the already open Windows IPC defect CS22/G11; it is not counted as a test failure
demonstrating this repair. The red logs above contain actual failed assertions.

## Test quality and required checks

Removed four low-value driver checks: two tests of the unused compatibility
protocol and two private range/roundtrip assertions. A public four-model raw-wire
fixture replaces the latter. The driver matrix checks every numeric field in
both batch APIs and proves no valid prefix escapes rejection.

Existing long controller replays had refreshed only the moving joint, silently
depending on the old global watchdog to cover stationary peers. Their fixtures
now provide a new stationary-peer observation on every simulated tick. Their
planner/stall assertions remain intact; no watchdog deadline was widened in
production. New admission fixtures use bounded synthetic timestamps instead of
waiting seconds for missing hardware. Controller startup checks inspect recorded
wire values, timing out after two simulated ticks.

Final integrated controller/Pi coverage passes 181 tests (158 existing Berthier,
five new controller contracts and eighteen Pi cases). Davout passes 88 tests
(67 existing, five prior output contracts, sixteen new admission cases). Config
passes 74 tests; Robstride's native final suite passes 36. The required command
`docker compose run --rm -e CI=true -e GITHUB_REF=refs/heads/main check` passes:
**547 Rust tests, one ignored; 355 frontend tests; 72 Pi MCP tests**. Format,
Clippy with warnings denied, proto/build, high-severity Consul audit, Cargo
deny/audit and the aarch64 release build complete. Summed Rust test-body time is
1.58 seconds; test compile/link is 7.78 seconds, Clippy 4.85 seconds and cross-build
9.20 seconds. These are separate stages, not a full wall-time benchmark.

Cargo audit reports the two known unmaintained dependencies (M01); Consul's two
moderate advisories remain T31. Cargo deny exits successfully but emits 344
registry index warnings: yanked-crate coverage is **unknown**, not clean. This
scanner-coverage gap remains recorded with T26. GitHub run `36666906670` passes
all five jobs at code commit `d4c869b`: changes, image, primary check, simulation
and Linux vCAN. The documentation-only evidence commit receives its own final
checks before merge; the exact head/run/merged SHA is preserved in the local
`batch02/merge-receipt.json` and the PR record.

Independent review of root bootstrap/overlay and of Davout/driver found and
closed the enable queue race. The unchanged external probe fails before that
repair and passes afterward. Final driver semantic-mode review is recorded
with the gate result. The taught-range zero-target regression was independently
identified, then reproduced against the actual controller and repaired without
relaxing Davout's hard-bound check.

## Remaining work and acceptance

CS04 still needs status fault bits, complete u32 detailed faults and persistent
fault authority; only fault-only pose timestamp handling is repaired here.
CS12 still discards post-send safety errors. CS13 still lacks explicit latched
recovery. CS14 rejects unsupported actions and requires explicit torque caps,
but zero-kd velocity clipping still does not establish a fall/braking response.
CS15 still exposes mutable raw configs, synthetic feedback and bus access;
immutable installed generations, schema-key strictness and coordinated live/
durable policy updates remain required. Validation is not an ownership rewrite.

CAN status has no command-generation identifier. Queue draining and original
decode timestamps provide a receive-time boundary, not proof that a late frame
was physically acquired after enable. Installed drive reporting/timeout behavior
needs separate verification. The static parameter table matches current RS02/
RS03 manuals, but older firmware/manual variants conflict; no runtime code
writes EPScanTime or CanTimeout yet. M02 retains model/firmware schema/readback
and hardware limit/timeout acceptance. No wire scales or caps were increased.

Scoped admission checks active joints; gravity still uses the loaded full model
with zero fallback for missing inactive poses. That existing model-scope
limitation remains with M05/M07 and does not gain acceptance from this batch.
Reference validity, physical E-stop/support behavior, measured stop outcome,
CAD/URDF/MJCF provenance and the incomplete commissioning ladder remain open.
Wave sign-off is unchanged.

## Docker recurrence

During testing all Desktop/backend processes disappeared and the Linux engine
pipe vanished. Logs and Windows events do not identify the initiating cause.
A hidden restart reproduced the supplied Inference socket error. The verified
ordinary runtime folders were preserved by timestamped rename and recreated;
startup and an independent health probe passed (Linux engine 29.6.1, twelve
running containers, twelve images and 236 volumes). Docker settings, virtual
disks, data and the existing launcher session were preserved.

The ordinary launching parent's exit does not reproduce the disappearance. This
is a verified recovery workaround, not a permanent fix or evidence that moving
Marengo caused it. Full local evidence is `batch02/docker-recurrence.md`; M08
records the remaining process-exit/Windows socket dependency. All repository,
CAD and recovery material remain under `J:\code`.
