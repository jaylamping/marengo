**1. Must-fix — Establish authority and one deletion gate. [Q1, Q3]**

**Where:** `roark/PLAN.md`, “Start here” and “Before the old stack is deleted”; `roark/TENETS.md`, opening paragraph; `roark/CLAUDE.md`, reference-repo instruction.

**Current:**

> “If a legacy doc and this plan disagree, this plan wins.”

> “once the left arm passes its tests, the right-arm-era code that wasn't carried over is deleted.”

> “the reference repo … is read-only from here.”

**Replace with:**

> These tenets and this plan govern software architecture and implementation order. They do not override hardware measurements, vendor protocol definitions, or unresolved safety requirements. Hardware STATUS.md identifies the current hardware specification; where it explicitly delegates to a detailed document, read that document. Record disagreements and keep the affected motion disabled until resolved.
>
> The reference repository remains read-only to agents working in this software repository. Legacy deletion occurs only after milestone 2 passes, the retained tools and commissioning evidence are available independently, and rollback has been demonstrated. The hardware-repository owner performs the deletion in a separate reviewed change. Passing milestone 1 or replacing one crate does not authorize earlier deletion.

**Why:** `PACKET.md` specifies deletion after milestone 2; `marengo/hardware/STATUS.md` explicitly gives linked detailed documents precedence; the current four entry documents disagree about timing and ownership.

---

**2. Must-fix — Separate fixed inventory from permission to energize. [Q1, Q2, Q3]**

**Where:** `roark/TENETS.md` §1; `roark/PLAN.md` §1 and “Milestones.”

**Current:**

> “When a drive answers it is registered and addressed; when it doesn't, it shows as offline. Nothing else.”

> “A limb that isn't built yet is just a limb whose drives are all offline. It needs no separate code path.”

> “Unbuilt limbs are listed and simply show offline.”

**Replace with:**

> The skeleton lists the fixed robot inventory. Discovery reports presence; it never assigns joints, changes hardware identity, or authorizes motion.
>
> Each run declares its required joints and mechanically attached joints before preflight. An absent joint outside that set does not block the run. An absent required joint blocks enable. Loss of a required joint after enable stops the run and invalidates its preflight; it must not silently become an optional offline joint.
>
> A commissioning fixture may select fewer joints than a completed limb only when the omitted hardware is physically absent or independently supported and the fixture configuration records that fact. The same admission and stop logic applies to every limb.
>
> Unexpected addresses, conflicting identity replies, or a changed drive identity block motion on the affected bus. A responding drive remains disabled until its identity, configuration, reference and safety checks pass.

**Why:** `marengo/review/sw-foundation-20261108/RECONCILIATION.md` identifies the legacy all-joints-online gate as a blocker, while `marengo/crates/robstride/src/identity.rs` provides identity evidence beyond mere presence.

---

**3. Must-fix — Make the skeleton an implementable contract without fabricating hardware values. [Q1, Q2, Q4]**

**Where:** `roark/PLAN.md` §1.

**Current:**

> “One file, `config/skeleton.yaml`, describes the whole robot…”

> “Directions for the left joints are unknown until the sign check; leave them as placeholders.”

**Replace the field-description paragraph and placeholder bullet with:**

> The software project owns the schema and validation; the hardware owner approves physical values. The following is the required schema contract, not a claim that these values have already been measured:
>
> - A schema revision and the robot display name.
> - The five limb identities and ordered joint membership.
> - For each bus: its logical identity, physical interface binding and commissioned adapter identity. Simulation supplies an explicit binding to `vcan`; it does not alter joint identity.
> - For each joint: its name, actuator model, bus, device ID, expected drive UID when commissioned, and the model/firmware evidence used to qualify it.
> - Direction, reference-pose definition, host zero offset, and whether direction and reference have been verified.
> - Separate mechanical travel bounds, approved operating bounds and soft bounds. Each bound records whether it is measured, provisional or unknown.
> - Separate torque, drive-current and speed limits, with units, operating-mode applicability and evidence. Protocol full-scale values are not operating limits.
> - Per-model protocol scales, stored once and supplied to the codec.
>
> Store angles in radians, angular velocity in radians per second and joint-output torque in N·m. Identify whether every current value is phase current, RMS current, peak current or DC supply current; do not use an unqualified “A.”
>
> Unknown values are explicit nulls or unverified values, never zero or an assumed sign. Unknown motion-critical values block ordinary enable for the affected joint. A narrowly scoped commissioning operation may establish the missing value under its own approved fixture limits.
>
> Reject duplicate joint names, duplicate addresses on one bus, unresolved bus references, non-finite values, invalid directions, unsupported model selections and incorrectly ordered limits. Device IDs may repeat on different buses. Reject unknown schema revisions and unknown fields rather than silently ignoring them.
>
> The whole inventory must load while unbuilt joints remain uncommissioned. Loading successfully does not mean that every joint is eligible for motion.

**Why:** `marengo/hardware/docs/decisions/0001-can-and-motors.md` defines address uniqueness per bus; `marengo/hardware/STATUS.md` marks several left-arm values provisional; `marengo/config/motors_humanoid.yaml` contains placeholder addresses that must not become commissioned facts.

---

**4. Must-fix — Define zero, sign and power-cycle validity. [Q2, Q3]**

**Where:** `roark/PLAN.md` §1, new subsection after the schema contract.

**Current:** No coordinate conversion or zero-validity contract.

**Add:**

> **Coordinate and reference contract**
>
> Let θ be the drive-reported output angle in its current firmware coordinate system, z the host offset expressed in that same system, and s the verified direction, either +1 or −1. Joint position is `q = s × (θ − z)`. Joint velocity and joint-output torque use the same direction convention. Commands apply the inverse conversion. The actuator’s internal gearbox reduction must not be applied again to an already output-side angle.
>
> The reference pose is a described physical pose, not merely “the encoder reads zero.” Angles remain continuous within the commissioned travel interval; do not wrap a discontinuity into a plausible pose.
>
> A host offset and a drive Set Zero are different operations. Normal startup never writes Set Zero. An explicit attended reference operation may write it, but must then establish the resulting coordinate system and corresponding host offset without applying the correction twice.
>
> A process restart, drive power cycle, unexpected Reset mode, identity change, feedback discontinuity or communication loss invalidates the affected joint’s reference and preflight. Rediscovery alone does not restore them. Re-establish the physical reference before position control or gravity compensation.
>
> Do not assume that a drive supplies a unique reboot notification or that its UID changes after reboot. Recovery must work when neither happens.
>
> Reference teaching produces a proposed skeleton diff. Runtime reference validity is session state; it is not permission to rewrite the skeleton.

**Why:** `marengo/config/motors.yaml` uses direction and unity external gearing; `marengo/crates/davout/tests/physical_firmware/mod.rs` models volatile zero; `marengo/docs/commissioning/firmware/robstride-firmware-behavior.md` leaves important Set Zero behavior unresolved.

---

**5. Must-fix — Resolve the skeleton-versus-dynamics contradiction. [Q1, Q2, Q3]**

**Where:** `roark/TENETS.md` §2; `roark/PLAN.md` §1.

**Current:**

> “The runtime reads that file and nothing else about the hardware.”

> “A URDF or MJCF for sim and visualisation may be generated from CAD or from the skeleton, but the runtime doesn't depend on it.”

> “URDF and MJCF are for sim and dynamics only.”

**Replace with:**

> The skeleton is the sole authority for joint identity, addressing, direction, reference convention and operating limits. The runtime never reads CAD or design source files.
>
> Dynamics may consume a reviewed, versioned model containing geometry, masses, centres of mass and inertia. That model must identify the skeleton revision and joint mapping it matches. It must not override skeleton limits or addresses.
>
> Runs that require gravity compensation or collision checking refuse to start without a matching, qualified model. Loose-drive protocol and safety tests do not require a dynamics model.
>
> Calibration changes are proposed revisions. Applying a geometry, mass, zero or sign change invalidates the affected run’s previous acceptance evidence.

**Why:** `marengo/hardware/designs/left-arm/urdf/README.md` describes a CAD-generated model with provisional geometry and masses; `marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md` requires that model for calibration and collision checking.

---

**6. Must-fix — Do not equate parameter readback with protection in MIT mode. [Q2, Q4]**

**Where:** `roark/TENETS.md` §3, drive layer; `roark/PLAN.md` §6, caps row.

**Current:**

> “Drives (holds even if the Pi crashes): CAN host-loss timeout of 50 ms or less, torque, current and speed caps…”

> “`limit_torque` 0x700B, `limit_cur` 0x7018, `limit_spd` 0x7017 | Per joint from the skeleton; never above the weakest printed part on that joint”

**Replace with:**

> Drive protection is qualified separately for each actuator model, installed firmware and control mode. Before enable, write the approved limits and read them back using the correct register types. Refuse missing, failed, stale or mismatching readbacks.
>
> A successful readback proves the stored value, not that the limit constrains every control mode. The manuals describe `limit_spd` 0x7017 as a CSP position-mode speed limit and `limit_cur` 0x7018 as a velocity/position-mode current limit. Their effectiveness in operation-control/MIT mode is unverified here. The applicability of `limit_torque` 0x700B to total MIT torque must also be demonstrated.
>
> Before relying on a cap for attached-arm motion, perform a bounded loose-drive or supported-fixture test showing that the installed firmware enforces it in the selected mode. Use a deliberately low test cap; do not test protection by approaching a motor or printed-part damage limit.
>
> Torque limits must respect both the actuator’s applicable duty rating and the mechanical load path. Current and speed limits require their own dimensionally correct limits. No mode is admitted merely because all three registers can be read.

**Why:** The four manuals’ §4.1.13 tables explicitly qualify current and speed by mode; `marengo/crates/davout/src/total_torque.rs` says the total MIT clamp is undocumented. Sources: `marengo/hardware/docs/robstride/RS00_User_Manual_260713.pdf`, `RS02_User_Manual_260713.pdf`, `RS03_User_Manual_261013.pdf`, `RS04_User_Manual_260713.pdf`.

---

**7. Must-fix — Correct timeout, damper and manual-conflict wording. [Q1, Q4]**

**Where:** `roark/PLAN.md` §§2 and 6.

**Current:**

> “Write 1000 (= 50 ms; 20000 = 1 s) and read back before every enable; refuse on 0 or above.”

> “Damper 0x702A (RS03 also 0x2026) | Read only; must read 0. Never write 1”

> “every RS00 manual says ±33 / ±14.”

**Replace with:**

> Write `canTimeout` 0x7028 as uint32 value 1000 and require a matching readback before enable. The documented conversion is 20000 counts per second, so 1000 counts is 50 ms. Reject zero, a value above 1000, a failed read or any value differing from the requested setting. Requalify after any suspected drive reset or power cycle. Test which received command types refresh this timeout.
>
> Read `damper` through the private-protocol parameter 0x702A and require value 0. “Read only” is a software policy: the vendor table describes the register as writable. Never write 1. The RS03 configuration-table address 0x2026 is not an interchangeable address for the type-17/type-18 parameter interface without evidence.
>
> The vendored RS00 manual’s type-1/type-2 tables specify velocity ±33 rad/s and torque ±14 N·m. Legacy code uses ±50 and ±17. Do not generalize this review to every manual revision or installed firmware. Qualify both command and feedback scaling, including torque against an independent load measurement, before attached RS00 motion.
>
> Preserve the RS02 manual inconsistency explicitly: its type-1/type-2 tables specify ±44 rad/s and ±17 N·m, while its §4.1.13 parameter table lists values resembling RS00, including a 14 N·m torque limit. Do not silently choose one table for register validation.

**Why:** These distinctions are visible in the four vendored manuals; legacy disagreement is explicit in `marengo/crates/robstride/src/motor_type.rs` and `marengo/docs/commissioning/firmware/robstride-mit-ranges.md`.

---

**8. Must-fix — Specify drive and run states, including partial enable. [Q2]**

**Where:** `roark/PLAN.md` §6, new subsection.

**Current:** No explicit state-transition contract.

**Add:**

> **State transitions**
>
> Track presence, reference validity, preflight validity, commanded enable state and observed drive mode separately. Do not compress them into one “online” flag.
>
> A run progresses through disabled, preflight, enabling, active and stopping. A safety failure enters a latched fault state. Each transition records its cause and evidence.
>
> Startup is disabled with no resumable motion. Preflight checks every required joint. Enabling is a bounded transaction: no motion plan starts until every required joint has entered the expected mode with fresh feedback. If any joint fails, stop every joint already enabled by that transaction.
>
> Unexpected Reset/calibration mode while active is a fault. Expected Reset while disabled or during a documented reference operation is not an active-motion fault. Those operations have their own bounded deadlines.
>
> Clearing a host fault, clearing a drive fault, establishing a reference and enabling are separate explicit actions. None implies another. No restart, reconnect, late reply, temperature recovery or restored power automatically resumes a run.
>
> A stop records separately: requested, transmitted, confirmed by fresh drive status, or unconfirmed. An unreachable drive remains unconfirmed; it must not be displayed as confirmed disabled.

**Why:** `marengo/crates/robstride/src/lifecycle.rs` distinguishes Disable and fault-clear payloads; `marengo/bins/motor-repl/src/stop.rs` explicitly warns that a successful write does not prove a stopped drive.

---

**9. Must-fix — Define enable and reference sequences without promoting legacy timing to universal truth. [Q1, Q2, Q4]**

**Where:** `roark/PLAN.md` §2 and §6.

**Current:**

> “Set Zero timing, enable and stop order…”

> “the drive model in `davout/tests/physical_firmware/`”

**Replace with:**

> Lift the evidence from `crates/davout/tests/physical_firmware/mod.rs` and `docs/commissioning/firmware/robstride-firmware-behavior.md` in the reference repository. Preserve each observation’s actuator, firmware, adapter and capture provenance.
>
> Before an enable attempt: confirm the required drives are disabled; remove inherited active reporting; drain and classify residual traffic; verify identities and references; write and read back the safety settings; and establish neutral initial commands and the intended run mode.
>
> The recorded legacy sequence sends active-reporting Off, observes its local echo and waits before Enable, then writes RunMode. Treat that ordering as a qualification test case, not a universal firmware guarantee. Demonstrate that transition from every permitted previous mode cannot execute a stale target. Refuse a mode/firmware combination until its neutral enable sequence is qualified.
>
> After an explicit Set Zero, retain the documented 800 ms exclusion before enable and the reporting-write exclusion from 450 ms to 800 ms. These are host guard intervals, not a claim that the blackout lasts 450 ms. Re-establish fresh identity, reference and parameter evidence after the interval.
>
> No sleeps or reference transactions block active control of another bus. Qualification traffic occurs only while the affected run is disabled.

**Why:** `marengo/docs/commissioning/firmware/robstride-firmware-behavior.md` documents delayed blackouts, mixed firmware and unresolved cases; the path currently quoted in PLAN omits `crates/`.

---

**10. Must-fix — Define fault-stop delivery and recovery when the bus is lost. [Q1, Q2]**

**Where:** `roark/PLAN.md` §6; `roark/TENETS.md` §3.

**Current:**

> “Stop the limb after 3 missed replies … never hold drives energised after a drive goes silent”

> “Stop on any fault bit; never auto-clear and retry”

> “Stop the whole robot; no automatic restart while moving”

**Replace with:**

> On a fault, unexpected drive mode, required-drive silence, invalid feedback or failed safety check, cancel the run and suppress all further motion commands before attempting stop delivery.
>
> Disable every enabled joint in the affected run, including healthy joints that were supporting a failed joint. Bus-off, interface disappearance, a control-loop failure or loss of shared safety state stops all active runs. A failed bus must not delay stop attempts on healthy buses.
>
> Fault stops do not attempt a rest-pose trajectory. Send type-4 Disable with the fault-clear byte unset; attempt reporting Off afterward. A failure to send Off must not prevent Disable. Auxiliary neutral or speed-zero writes must not delay Disable and must not be described as proof that torque is off.
>
> Bound stop-write retries and record their outcomes. Do not wait for logger flush, storage, UI, a parameter read or a reply from another drive before attempting the remaining stops.
>
> If CAN cannot deliver the stop, suppress all traffic that could refresh the affected drive’s host-loss timeout and rely on the qualified drive timeout. When the power board is present, also stop its heartbeat when the fault requires removal of motor power. Without that board, software cannot claim to have cut power.
>
> Recovery requires disabled state, cleared stale transmit work, restored transport, explicit operator review and a complete new preflight. Bus recovery never replays old commands.

**Why:** `marengo/bins/motor-repl/src/stop.rs` already distinguishes send success from physical confirmation and continues across failed interfaces; `marengo/review/sw-foundation-20261108/RECONCILIATION.md` identifies shutdown-before-disable and degraded hold as failures.

---

**11. Must-fix — Prevent bus threads from keeping a crashed controller’s commands alive. [Q2]**

**Where:** `roark/PLAN.md` §§3 and 6, “Hang.”

**Current:**

> “systemd `WatchdogSec` fed from the control thread; later the power board heartbeat”

**Replace with:**

> Every command batch carries the runtime session, control tick and a monotonic expiry. A bus thread transmits only a fresh batch from the current control session. It must not repeatedly send the last batch while waiting for the control thread.
>
> Missing or expired control work stops motion traffic, including diagnostic traffic that could keep a drive watchdog alive, and initiates the fault-stop path. Queued commands from an old session are discarded.
>
> Feed the systemd watchdog only while the control loop and every bus required by an active run demonstrate timely progress. A live control thread must not hide a hung bus thread. Feed the later power-board heartbeat from the same aggregate health decision, never from an independent timer.
>
> Test separately: control-thread stall while bus threads run; one bus-thread stall while control runs; whole-process termination; process restart; and a blocked logger. The accepted result is no stale command replay and no automatic enable after recovery.
>
> systemd supervision and `ExecStopPost` are additional protections. They do not replace the drive timeout and do not run when the Pi has lost power.

**Why:** PLAN introduces independent bus threads, while `marengo/scripts/systemd/marengo-pi.service` only provides an exit hook; `marengo/hardware/electrical/pdb/REV_B_PARTS.md` requires heartbeat generation tied to control-loop health.

---

**12. Must-fix — Turn “total torque bounded” and soft margins into real admission rules. [Q1, Q2]**

**Where:** `roark/PLAN.md` §6, “Limits”; §8.

**Current:**

> “total commanded torque bounded, backed by the drive's own limit”

> “Soft limits 2 to 3° inside hard stops supervised, ≥ 5° unattended”

> “Elbow torque cap | ≤ 17 N·m … lower if the rig fails low”

**Replace with:**

> Bound the complete MIT expression `Kp × position_error + Kd × velocity_error + feed_forward`, including gravity and friction contributions. Evaluate the final encoded command using feedback age, quantization and the qualified motion/transport uncertainty. Do not validate feed-forward alone or an intermediate command that later changes.
>
> If a valid bound cannot be established, reject the command and stop the affected run. Reducing a gain or setpoint is acceptable only when the resulting complete bound is demonstrably within the approved cap. A host prediction does not establish a physical torque guarantee between samples; the qualified drive protection remains required.
>
> The 2–3° supervised and at least 5° unattended insets are minimum geometric margins. Approve speed and trajectories only when the measured stopping travel and uncertainty remain inside those margins. Check measured motion as well as requested targets. Do not assume a drive speed-limit register enforces this in MIT mode.
>
> The elbow’s 17 N·m value is a printed-stop design load and the RS02 peak rating, not an approved continuous command. Use the lower applicable actuator-duty, validated mechanical and run-specific limits. A failed dummy-rig gate requires hardware review and retest; software must not substitute an invented lower cap for a failed gate.
>
> Include loads transmitted from other joints and payloads. An elbow actuator cap does not bound shoulder-induced load on the elbow stop.

**Why:** `marengo/crates/davout/src/total_torque.rs` admits cases where its predicted bound remains above the cap; `marengo/hardware/designs/left-arm/testing/ELBOW_DUMMY_OUTPUT_TEST.md` separates the 17 N·m design case from shoulder-through-stop loading and mandates rework after failure.

---

**13. Must-fix — Make temperature, voltage and camera stops executable. [Q1, Q2]**

**Where:** `roark/PLAN.md` §6 and §9.

**Current:**

> “VBUS … 22 to 26 V on the bench supply; no faults present”

> “Pause at 60 °C, stop at 75 °C”

> “Reuses item 6's preflight.”

**Replace with:**

> Temperature and supply-voltage checks run throughout motion, not only at preflight. The run declares the power source and required sensor freshness; stale or failed safety measurements are failures.
>
> On the 24 V bench supply, VBUS outside 22–26 V stops the run. This is a project operating window, not the vendor’s full operating range, and must not be reused for battery operation.
>
> At temperature ≥60 °C, end motion and enter a bounded cooling procedure. A move to a rest pose is allowed only while every required drive and sensor remains healthy and the path is already validated; otherwise disable. Holding torque indefinitely is not cooling. Re-arm only after temperature is ≤45 °C and an explicit resume passes preflight. At ≥75 °C, take the fault-stop path.
>
> For a camera-dependent stage, loss of the frame tag, a required tag missing at a measurement hold, or frames stale for more than 1 s stops the stage. The host runner enforces this even if the agent or camera client crashes.
>
> Copy the remaining stage-specific residual, torque and collision checks from CALIBRATION_LOOP.md into the executable stage contract. Give each measurement a deadline and each stop procedure a bounded completion condition before admitting that stage.

**Why:** `marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md` §9 supplies these triggers and the 45 °C recovery threshold; PLAN currently omits recovery and continuous monitoring semantics.

---

**14. Must-fix — Make CAN timing a worst-case budget. [Q1, Q2, Q4]**

**Where:** `roark/PLAN.md` §3.

**Current:**

> “Each tick: send every command, collect replies until about 60 % of the period…”

> “Parameter traffic only in the slack.”

> “250 Hz default; 300 Hz only if measured.”

> “short TX queue.”

**Replace with:**

> The default period is 4 ms, derived from 250 Hz. The initial receive deadline is 2.4 ms from tick start, derived from the research report’s 60% allocation. Treat that allocation as a measurement candidate, not a demonstrated Pi timing guarantee.
>
> Every transmit, receive and parameter operation has a deadline. “Blocking socket” does not permit an unbounded blocking write or read. The realtime path performs no allocation, disk I/O or wait on a non-realtime consumer.
>
> Parameter traffic is admitted only when its request, response and queue occupancy fit the remaining budget without delaying the next cyclic exchange. Stop and fault traffic takes priority.
>
> Measure wakeup lateness, last-command transmission, reply completion, feedback age, queue occupancy, ENOBUFS, receive drops and stop-delivery latency under the largest bus population and concurrent camera/logging load required by the milestone. Report worst observed values as well as distributions.
>
> Three missed cyclic replies is an active-run stop threshold, not a substitute for elapsed-time checking. At 250 Hz, three periods are 12 ms. A stalled scheduler must fault immediately on waking with expired feedback; it must not wait three additional ticks.
>
> Use the research report’s initial Linux settings: 1 Mbit/s, sample point 0.75, error reporting enabled, transmit queue length 10 and automatic bus-off restart disabled. Validate them on the actual adapter.
>
> A 300 Hz full-limb mode or approximately 1 kHz single-drive stand mode requires its own measured budget and fault tests. Neither is an automatic extension of a passing 250 Hz test.

**Why:** `marengo/hardware/docs/research/can-stack-for-marengo.md` supplies the settings and explicitly labels bus-load arithmetic as unmeasured; `marengo/docs/commissioning/firmware/robstride-firmware-behavior.md` documents delayed and lost replies.

---

**15. Must-fix — Specify receive validation and the limits of reply correlation. [Q2]**

**Where:** `roark/PLAN.md` §3, new codec/receive contract.

**Current:**

> “a typed frame table transcribed from the RobStride manuals”

**Add:**

> The frame table specifies identifier layout, payload length, byte order, field ranges, register types and allowed state transitions for every implemented message.
>
> Reject malformed lengths, inappropriate standard/RTR/error frames, invalid numeric values, foreign-host replies and unexpected addresses before using them as joint state. Classify local transmit echoes, identity replies, version replies, parameter replies, type-2 status, active reports and type-21 faults separately.
>
> In particular, a type-2 version response beginning `00 C4 56` is not position feedback. Any reported fault, including an unknown nonzero fault bit, is retained and handled conservatively; do not invent a precise fault label from the manuals’ duplicated bit description.
>
> Parameter reads correlate bus, device, host, index, result status and the current transaction. Permit only one outstanding transaction with the same correlation key.
>
> The cyclic protocol does not provide a host tick sequence number. Arrival order alone cannot prove that a delayed reply belongs to the latest command. Record receive age and correlation uncertainty; queued, duplicate or unsolicited traffic must not manufacture fresh command-response evidence.
>
> Use independent manual/capture vectors as well as codec round trips. A simulator using the same codec cannot independently prove that codec’s correctness.

**Why:** `marengo/crates/robstride/src/identity.rs`, `params.rs`, `lifecycle.rs`, and the manuals’ §4.1 tables distinguish these messages; the shared-codec simulator otherwise permits matching encoder/decoder bugs.

---

**16. Must-fix — Define the single runtime interface and motion authority. [Q1, Q2]**

**Where:** `roark/TENETS.md` §5; `roark/PLAN.md`, `api`, `robot-cli`, `ui/` and `mcp/` rows.

**Current:**

> “Everything else … talks to the realtime process through one interface, and can crash without affecting motion.”

**Replace with:**

> The initial runtime interface is one local Unix-domain socket carrying versioned, length-delimited protobuf messages. This is a proposed software contract, not an existing implementation. The UI bridge, CLI and MCP are clients of that interface and never open a motion-capable CAN socket.
>
> The interface supports status subscription, discovery, commissioning operations, preflight, explicit enable, submission of a finite run plan, run status/results, stop/disable and explicit fault acknowledgement. Add another operation only when a scheduled test needs it.
>
> Every mutating request identifies the client, runtime session, request identity, target run/joints and expected skeleton revision. Reject stale-session requests, duplicate execution, unknown joints, conflicting runs and requests that bypass state checks. A reconnect may retrieve an existing result; it must not repeat a physical action.
>
> Only one run may command a joint at a time. Stop is accepted independently of motion ownership and has priority over queued work. Socket permissions restrict which local principals may command motion; the bridge must not expose an unauthenticated remote motion endpoint.
>
> Finite plans execute inside the runtime with their stop rules. Losing the submitting UI or agent does not remove those rules. A plan may continue after client loss only when it was admitted as autonomous and all required sensing remains healthy. Any future live-command mode must expire when fresh commands stop arriving.
>
> Client or logger failure must not block the realtime loop. Failure of a dependency required by the active run causes the specified stop.

**Why:** `marengo/review/sw-foundation-20261108/reviews/claude-opus.md` proposes a local protobuf interface; `marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md` §10 requires host-executed plans rather than live agent streaming.

---

**17. Must-fix — Close CAN ownership and adapter-binding holes. [Q1, Q2, Q3]**

**Where:** `roark/PLAN.md` §3 and §5; `roark/TENETS.md` §5.

**Current:**

> “Only the realtime process transmits. The one exception is the independent `disable` run by systemd after the process exits.”

> “bus names come from the skeleton, so moving a limb from the HAT to a USB-CAN adapter is a config change.”

**Replace with:**

> Scan, set-ID, reference and diagnostic requests execute through the runtime, including while motion is disabled. `robot-cli` is their client.
>
> The sole direct-transmit exception is the minimal systemd exit-stop helper after the runtime has terminated. It sends only Disable and reporting Off to the commissioned addresses, attempts every bus independently and reports failed delivery. Service restart waits until this helper finishes. The helper does not depend on the dynamics model, UI, logging service or complete motion-configuration validation.
>
> A second runtime must fail ownership acquisition before transmitting. Passive capture remains allowed.
>
> Bind logical buses to stable adapter identity rather than USB enumeration order. Verify binding and drive identity before enable, including after adapter reconnection. Moving from a HAT to USB requires configuration and deployment binding changes plus timing requalification; it does not require a new control-code path.
>
> Set-ID is an attended commissioning transaction with one physical drive connected on the selected bus. Read its UID before the change, verify the same UID at the new address afterward, then propose the skeleton update. A scan is an observation and cannot guarantee discovery of conflicting devices sharing one address.

**Why:** `marengo/bins/motor-repl/src/stop.rs` demonstrates the minimal stop dependency boundary; `marengo/hardware/electrical/wiring/left-shoulder-bench-harness.md` requires one-drive-at-a-time ID assignment.

---

**18. Must-fix — Specify logging without letting storage control motion timing. [Q1, Q2, Q4]**

**Where:** `roark/TENETS.md` §8; `roark/PLAN.md`, “Observability.”

**Current:**

> “Detailed, concise, timely logs at every step…”

> “Every frame in and out goes to MCAP from the realtime process … with kernel timestamps”

> “Structured, timestamped events and metrics from every layer, on one shared clock…”

**Replace with:**

> Realtime threads enqueue bounded records; a non-realtime recorder writes MCAP. Record raw CAN traffic, decoded state, commands before and after admission, parameter transactions, faults, state transitions, operator actions, run results and injected faults.
>
> Each raw record identifies the session, bus, direction, CAN identifier and flags, payload length and bytes, monotonic observation time, available kernel/adapter timestamp and its provenance. Distinguish command enqueue, successful socket write, local echo and received drive response. A candleLight transmit echo is not proof of on-wire completion.
>
> Decoded samples retain their source-frame identity, model/scales, joint units, validity and age. Record control tick and request/run identity so commands can be related to outcomes.
>
> A session is one directory containing the admitted plan, exact configuration/model revisions or immutable copies, MCAP recordings, per-bus candump logs, selected camera frames and the result summary. Choose concrete filenames in the recorder implementation and document them in README; no second database or custom archive service is required.
>
> Record drops and discontinuities explicitly. Never block control to preserve a log. Recorder failure, disk full or ring overflow makes the session incomplete and fails any measurement or calibration gate that requires the missing evidence; stop that run using its defined stop policy.
>
> Use monotonic time for deadlines and feedback age. Record wall-clock mappings and synchronization quality for cross-host replay. A wall-clock adjustment must not alter a safety deadline.
>
> Run passive `candump -L` independently, with bounded storage and reported failures. It is an additional capture path, not guaranteed survival of host power loss. Prometheus/Grafana export runs outside realtime threads.

**Why:** `marengo/hardware/docs/research/can-stack-for-marengo.md` specifies the ring-buffer design and approximate candleLight TX timestamps; `marengo/hardware/docs/sim-to-real-loop.md` requires replayable sessions and clock synchronization.

---

**19. Must-fix — Make simulation independent enough to expose failures. [Q1, Q2, Q6]**

**Where:** `roark/PLAN.md` §4.

**Current:**

> “one sim-drive process … through the same codec, driven by MuJoCo, with fault injection built in…”

> “There are no spare actuators, so every stop rule is proven here first.”

**Replace with:**

> First build a deterministic protocol-level drive emulator on `vcan0`–`vcan4`. It models identity, enable/disable, run mode, parameter reads/writes, volatile state, host-loss timeout, active reporting and the documented Set Zero timing cases. MuJoCo coupling is required when a test needs physical motion or replay, not for the first protocol and stop tests.
>
> A scenario declares initial drive state, installed-firmware behavior being modeled, fault schedule or seed, expected transitions and expected output. Record the scenario and actual injection times.
>
> Cover lost and delayed commands separately from lost and delayed replies; malformed and wrong-host frames; stale feedback; unexpected Reset; a power cycle that clears timeout and limits; failed or mismatching parameter readback; ignored Enable/Disable; and startup with active reporting already on.
>
> Exercise control-process death, individual thread stalls, logger blockage and interface loss in addition to simulated drive faults. Verify both the stop decision and suppression of subsequent motion traffic.
>
> MuJoCo coupling defines the joint mapping, sign/zero conversion, simulated timestep, command hold behavior and treatment of an unavailable or stalled physics process. A stalled physics process must not return fresh timestamps on stale state.
>
> `vcan` proves software behavior under modeled faults. It does not prove physical bus capacity, electrical isolation, real controller bus-off recovery, motor-limit enforcement or a safe limp fall. Those require the corresponding hardware acceptance tests.

**Why:** `marengo/hardware/docs/research/can-stack-for-marengo.md` explicitly says vcan lacks bit timing; `marengo/crates/davout/tests/physical_firmware/mod.rs` is a timing/firmware model rather than evidence of real safety behavior.

---

**20. Must-fix — Reconcile test-stand stall limits and the blanket stall prohibition. [Q3, Q4]**

**Where:** `roark/PLAN.md` §7 and “Rules that hold for every item”; §8 “Never.”

**Current:**

> “stalled holds stop at the stall rating (RS03 13 N·m, RS02 6 N·m; check RS00 and RS04)”

> “torque cap = test maximum + 10 %”

> “Nothing overloads, overheats or stalls a real drive.”

> “Never … hold a stall…”

**Replace with:**

> Ordinary robot motion must not intentionally stall a drive. The explicit exception is an attended, supported actuator-stand test with a defined load, duration, temperature stop and torque/current cap.
>
> The vendored manual stall tables identify rated loads of RS00 3.6 N·m, RS02 6 N·m, RS03 13 N·m and RS04 28.5 N·m. These are vendor test-condition ratings, not automatically qualified continuous ratings for the printed fixture or installed robot. In particular, RS04’s 40 N·m rotating rating is not its stall rating.
>
> Set each test maximum from the lower applicable motor-duty, fixture and thermal limit. The proposed 10% control headroom must never raise the drive cap above that approved ceiling; lower the commanded test maximum if headroom is required.
>
> T5 stops at 60 °C or its approved duration limit, whichever occurs first. Do not run until thermal equilibrium if that would exceed the admitted duration or duty.
>
> Preserve separate rotating and stalled test cases and their mounting/cooling conditions in the report.

**Why:** I read the embedded stall tables in `marengo/hardware/docs/robstride/RS00_User_Manual_260713.pdf` (PDF page 11), `RS02_User_Manual_260713.pdf` (page 11), `RS03_User_Manual_261013.pdf` (page 12), and `RS04_User_Manual_260713.pdf` (page 12); `marengo/hardware/docs/sim-to-real-loop.md` currently conflicts with its own blanket prohibition.

---

**21. Should-fix — Define what T1–T6 actually deliver. [Q1, Q2, Q4]**

**Where:** `roark/PLAN.md` §7.

**Current:**

> “command and reply at about 1 kHz, logging command, position, velocity, current …”

> “Output: one file per actuator type, ready for MJCF `frictionloss`, `damping`, `armature`.”

**Replace with:**

> Approximately 1 kHz is a single-drive command/reply measurement target, not an established capability or the required rate of every channel. Prove the rate on the Pi and selected adapter with the complete test traffic.
>
> `iqf` 0x701A requires parameter traffic; ordinary type-2 feedback does not contain that current measurement. Budget its reads and record its actual sample times and gaps. Do not label interpolated current as a per-command measurement.
>
> T2 records scale reading, effective moment arm, lever angle, tare and fixture self-weight. T3 reports fixture-plus-drive compliance unless fixture deflection has been separated. T4 distinguishes host/USB latency from physical response and reports timestamp uncertainty.
>
> Each fitted result records drive UID/model/firmware, fixture and cooling conditions, temperature, applicable operating range, raw-session references, fit method and residuals. Report spread only when repeated measurements support it; one unit is not evidence of between-unit variation.
>
> A fit is accepted only after a separate no-refit replay or validation run passes its declared error gate. Failed or unidentifiable fits remain absent rather than receiving guessed coefficients.
>
> Implement RS03 first, then RS02 and RS00 as hardware becomes available. RS04 qualification remains required before leg use but does not block left-arm milestone completion.

**Why:** `marengo/hardware/docs/sim-to-real-loop.md` §§2.3–2.5 defines the experiments; `marengo/hardware/designs/actuator-test-stand/README.md` explicitly includes fixture compliance and effective lever geometry; the manuals identify `iqf` separately from type-2 feedback.

---

**22. Must-fix — Carry the unattended-calibration gates into the handoff. [Q1, Q2, Q3]**

**Where:** `roark/PLAN.md` §9.

**Current:**

> “camera service and stage C0 … then … C1 and C2 attended with Joey.”

**Replace with:**

> Implement C0 first, unpowered. A camera-dependent powered stage is admitted only after C0 passes and the host runner enforces the applicable safety rules.
>
> C1 and the first C2 run are attended. An unattended stage requires a clean attended pass of that same stage with the same code, configuration and hardware, an owner-supplied forbidden-pose list, validated paths and the required catch/support arrangement. Any relevant change invalidates that authorization.
>
> C5 remains attended. Power cycling invalidates reference and drive preflight; the test succeeds only when the reference procedure restores the required physical-pose repeatability.
>
> Translate CALIBRATION_LOOP.md §8 into executable gate definitions, retaining which thresholds are estimates. Record thresholds before each run. Do not relax a failed gate after observing the result and then call the same run a pass.
>
> Calibration tools propose zero, geometry, friction and COM changes. The runtime does not apply them to authoritative configuration during a run. After an approved revision is loaded while disabled, rerun the relevant stage without refitting.
>
> C6 reuses the measurement and fit gates of the right-arm suite, not its “Pi first” configuration-write workflow. Gestures and later stages are separate increments; C1/C2 completion does not imply C0–C8 completion.

**Why:** `marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md` §§7–10 requires these gates, while `marengo/docs/commissioning/right-arm-calibration-suite.md` still instructs Pi-first configuration mutation.

---

**23. Must-fix — Replace the abbreviated power-board requirement with explicit interlocks. [Q1, Q2, Q3, Q4]**

**Where:** `roark/PLAN.md` §10.

**Current:**

> “heartbeat toggled … port-fault reporting … self-test … brake clamp caps … brown-out budget, E-stop input debounce. Can be built against the sim now.”

**Replace with:**

> Implement this interface when power-board bring-up requires it, and complete its simulated fault tests before battery-powered motion.
>
> - Toggle WD_FEED from aggregate control health every 50–100 ms. The hardware requirement is a falling edge at least every 270 ms; the documented watchdog trip range is 270–355 ms. Test loss of heartbeat and manual-reset behavior on the actual board.
> - Treat ESTOP_PI_N as information only, with no GPIO pull and invalid state until configured. Debounce it without placing software in the hardware E-stop path.
> - On PORT_FLT_N, stop affected motion, record meter evidence and report the suspected port. Do not claim certainty when required meters are unavailable. U12 measures the right-arm port input, so fault localization is asymmetric.
> - Use one dedicated Pi I²C controller and the INA237 map in REV_B_PARTS.md. Limb and 12 V shunts are 2 mΩ; SPARE_SW is 5 mΩ with ADCRANGE 1. Integrate energy in software and record missing samples.
> - Run the bank self-test only with banks open, VBUS below 3 V, at least 5 s after the last stop and BMS pack voltage at least 40 V. Apply the board-calibrated MID-divider correction. Healthy corrected MID is 0.33–0.62 of pack voltage; the other classifications and failures follow REV_B_PARTS.md. A failed or unavailable self-test blocks software enable; the Pi cannot prevent a physical reset.
> - Refuse enable during charging or stale BMS telemetry. On minimum cell ≤3.0 V, filtered pack ≤38 V or BMS telemetry loss exceeding 5 s, stop motion before orderly compute shutdown.
> - Enforce the measured-pack-resistance brown-out budget and the board’s continuous, peak, connector and port constraints. Do not equate summed phase-current register values with DC pack current.
> - Limit braking to about 5 A when the pack is nearly full or cannot accept charge. Track total clamp energy using duty × VBUS² / 10 Ω, with duty derived from U17 shunt voltage /120 mV. Until the heat test replaces it, enforce 30 W average over a rolling 60 s and 3 kJ per burst.
> - Define the end of a braking burst and conservative handling of restart or missing clamp telemetry before enabling this feature. Restarting the process must not erase a hot resistor’s energy history and permit another full burst.
> - At an energy limit, execute a tested stop that respects the remaining braking capacity. “Stop braking and make a safe stop” is not an executable procedure by itself.
>
> Rev B5 has unfitted leg ports; leg power requires the hardware’s rev C decision and qualification. Passing the arm power-board milestone does not qualify leg operation.

**Why:** These requirements and the rev B5/rev C distinction are in `marengo/hardware/electrical/pdb/REV_B_PARTS.md`; `marengo/hardware/STATUS.md` also requires charging and BMS-staleness interlocks omitted from PLAN.

---

**24. Must-fix — Replace milestone slogans with acceptance evidence. [Q1, Q2, Q5]**

**Where:** `roark/PLAN.md`, “Milestones.”

**Current:**

> “Each milestone is ‘these drives come online and pass their tests’. None of them adds a code path of its own.”

**Replace the introductory sentence and “Proves” column with:**

> A milestone passes only with a recorded configuration, hardware revision, software revision, test results and unresolved limitations. The same control and safety implementation serves every limb; fixture and joint data may differ.
>
> | Milestone | Acceptance |
> |---|---|
> | 1 — Left arm | Skeleton validation, protocol vectors, simulated failure matrix, stop supervision and recording pass before motion. Loose-drive identity, timeout, cap and timing qualification pass. Required stand characterization passes for the admitted tests. Mechanical assembly gates pass before attached motion. C0, C1 and C2 pass on the assembled arm. Later camera stages remain separately gated. |
> | 2 — Right-arm migration | Existing right-arm identities, signs, references, limits and calibration provenance are reconciled. Both arms pass independently and concurrently. Loss of one required drive, one bus, the runtime and a required sensor produces the specified stop without stale replay or automatic recovery. Retained tools and old-session interpretation work independently of deleted components. Only then request legacy deletion. |
> | 3 — Waist | Waist address, geometry, support and limits are commissioned; the three-bus system repeats timing and failure tests. Passing an arm test does not qualify waist mechanics. |
> | 4 — Power board and battery | Actual board/BMS telemetry, heartbeat loss, self-test, charging inhibit, port fault, stale telemetry, brown-out and clamp-budget behavior are demonstrated. This milestone must precede any battery-powered motion, regardless of limb order. |
> | 5 — Legs | Approved leg power hardware, commissioned leg data and the support gantry are present. Five-bus load and fault tests pass with all required traffic. First motions occur supported; standing and walking require their own acceptance gates. |
>
> Owner confirmation is still needed for milestone 1’s camera depth. The proposed default above is C0–C2, not an implied claim that the full unattended C0–C8 programme is complete.

**Why:** `marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md` defines distinct stages; `marengo/hardware/docs/sim-to-real-loop.md` separates supported leg tests from locomotion; `PACKET.md` fixes migration before deletion.

---

**25. Must-fix — Put safety, logging and recovery before the first powered step. [Q5]**

**Where:** `roark/PLAN.md`, numbered “Start here” list.

**Current:**

> “Set up the new crate layout … and write the skeleton YAML…”

> “Build the new CAN layer and its simulated drive together…”

> “Buy one USB-CAN adapter … and bring the left drives up…”

> “Then the actuator test stand tooling … and the camera loop…”

**Replace the numbered list with:**

> 1. Read TENETS, this plan, hardware STATUS and its applicable detailed specifications.
> 2. Extract the cited protocol, firmware and commissioning evidence before implementing replacements. Record unresolved hardware facts.
> 3. Define the skeleton, coordinate conventions, run states, API authority, stop semantics and log records. Create only the implementation pieces required by the next acceptance test.
> 4. Build codec vectors, the deterministic vcan emulator, runtime admission and stop behavior together. Include process/thread failure, restart, stale queues and power-cycle scenarios.
> 5. Add the minimal CLI, MCAP/candump capture and systemd stop/supervision path. Demonstrate that logs and clients cannot block a stop.
> 6. Commission one USB-CAN adapter and the isolated bench harness. Establish identity and ID assignment with motion disabled, then qualify timeout, mode transitions, caps and Pi timing using a secured loose drive.
> 7. Run the needed actuator-stand tests and sign/reference procedures.
> 8. Verify mechanical assembly gates and approved limits before powering printed parts.
> 9. Complete C0 unpowered, then C1/C2 attended. Admit later stages only through their individual gates.
> 10. Migrate and qualify the right arm before deleting any legacy dependency.
>
> Adapter procurement may proceed while software work runs. It does not advance the motion gate.

**Why:** The existing sequence omits an explicit pre-motion gate for items 6 and observability; `marengo/review/sw-foundation-20261108/RECONCILIATION.md` lists drive protection and hung-process handling as blockers before powered motion.

---

**26. Must-fix — Preserve right-arm evidence without copying its faults. [Q2, Q3]**

**Where:** `roark/PLAN.md`, “Before the old stack is deleted.”

**Current:**

> “Move the right arm into the skeleton as a limb on `can0` … Then delete per the reconciled table.”

**Replace with:**

> Include the right arm in the skeleton inventory from the first revision. Milestone 2 commissions it on the new stack; it is not the point at which its identity first enters the schema.
>
> Reconcile the live `config/motors.yaml`, `config/control.yaml`, `config/homing.yaml`, the right-arm model and commissioning records. Preserve source values and provenance; do not blindly copy the old effective limits or tuning.
>
> The reference documents identify an incorrect right lower-arm yaw model axis, COM fits potentially contaminated by shoulder rubbing, conflicting limit authorities and unresolved RS00 scales. Resolve each affected item before relying on it for motion.
>
> Demonstrate the new reference workflow, sign checks, drive safety qualification, ordinary motion and fault stops on the right arm. Then demonstrate both arms concurrently.
>
> Preserve `gravity-fit`, `firmware-timing`, their input interpretation, relevant captures/profiles and commissioning evidence before deleting their dependencies. Validate retained tools on a known recorded session.
>
> Rollback restores the last reviewed software/configuration pair while disabled. It never resumes a prior trajectory or bypasses the new drive-protection gate.

**Why:** These specific migration hazards are documented in `marengo/hardware/docs/kinematics.md`, `marengo/config/motors.yaml`, `marengo/config/control.yaml`, and `marengo/docs/commissioning/firmware/robstride-mit-ranges.md`.

---

**27. Should-fix — Add a minimal build, CI and deployment contract. [Q2, Q6]**

**Where:** `roark/PLAN.md`, after “Names and crate layout”; `roark/CLAUDE.md`, final paragraph.

**Current:**

> “Work on branches and open PRs. No tests written just for coverage.”

**Add:**

> Pin a Rust toolchain and dependency lockfile in the new repository. The reference pins Rust 1.88.0; compatibility with the selected new dependencies has not been established by this review. Choose and record a compatible version rather than inheriting the pin silently.
>
> Required checks are formatting, compilation/lints, skeleton validation, independent protocol vectors and the relevant safety-contract tests. Linux integration CI must exercise the unchanged runtime against vcan; an unavailable vcan environment is a failed required check, not a skipped pass.
>
> Build and verify a Pi-compatible release. Deployment records the binary revision, configuration revision, model revision, target OS/kernel, adapter firmware and service settings. Install only while motion is disabled. Startup always returns to disabled state and requires fresh admission.
>
> Configure realtime priority, memory-lock permission, CPU affinity and CAN binding explicitly. If required realtime setup fails, report it and refuse powered operation until the measured operating policy is satisfied.
>
> Provide a repeatable disabled-state deploy and rollback procedure. Do not copy the legacy self-update service or CI container machinery unless a current requirement needs it.

**Why:** `marengo/rust-toolchain.toml`, `marengo/Cargo.toml`, `marengo/.github/workflows/ci.yml`, and `marengo/scripts/systemd/marengo-pi.service` establish the legacy choices but do not establish the new stack’s compatibility or deployment behavior.

---

**28. Should-fix — Correct readiness labels and preserve hardware gates. [Q1, Q3, Q4]**

**Where:** `roark/PLAN.md`, “When the hardware is ready” and §5.

**Current:**

> “Ready now (drives owned, bench harness planned)”

> “6808 bearings arrive Mon 2026-11-09”

> “120 Ω at each end … one ground tie.”

> “3 A fuse for first power-up, 10 A after.”

**Replace with:**

> Drives are owned; the harness and powered-test setup are not yet verified ready. Before first power, record the actual adapter, termination measurement, polarity/continuity checks, fuse installation, E-stop operation and secured fixture.
>
> The buy list says the initial 6808 pair is ordered for Monday delivery. Treat 2026-11-09 as the plan’s expected date, not evidence of receipt. Mechanical gates depend on inspected parts and completed tests, not the calendar.
>
> Preserve 120 Ω termination at each physical bus end and the approximately 60 Ω power-off measurement. Adapt the HAT-era grounding instructions to the selected isolated USB adapter: identify its CAN-side reference and avoid unintentionally bypassing galvanic isolation.
>
> Use the harness plan’s 3 A first-power fuse. Change to 10 A only after its specified individual-drive and combined-bus checks pass; it is not an automatic second step.
>
> Before printed-arm motion, require the connector fit, completed fastener stacks and unpowered harness sweep specified in the harness plan. Before forearm operation, require the applicable dummy-rig acceptance evidence.

**Why:** `marengo/hardware/electrical/wiring/left-shoulder-bench-harness.md` calls itself provisional and lists these gates; `marengo/hardware/designs/left-arm/BUY_ORDER.md` states Monday delivery but does not prove receipt.

---

**29. Should-fix — Replace remaining non-testable tenet language and remove unnecessary process. [Q1, Q6]**

**Where:** `roark/TENETS.md` §§0–8; `roark/PLAN.md`, introduction and layout section.

| Current text | Exact replacement |
|---|---|
| “Never for the current iteration of one limb.” / “If the answer is ‘only for this limb’ or ‘only for now’, it's the wrong change.” | “Shared runtime behavior must work for the fixed whole-robot inventory. Limb-specific hardware data, fixtures and acceptance tests are allowed. Implement only the subset of behavior needed by the current milestone.” |
| “Nothing in the code, the docs or the folder layout is named after the current step.” | “Name shared components by function. Limb names are appropriate for physical data, calibration evidence and fixtures that actually belong to that limb.” |
| “No mix-and-match of actuator types per limb…” | “Actuator assignment is fixed per joint. Do not implement runtime actuator substitution or a generic motor-plugin system.” |
| “When the hardware changes, the hardware side edits the skeleton, in one commit…” | “The hardware owner supplies and approves a proposed skeleton change. Commit it in the software repository and link the corresponding hardware STATUS update; cross-repository changes are linked revisions, not one atomic commit.” |
| “If a joint name, limit or ID appears in two files, one of them is wrong.” | “Each operational fact has one authoritative editable source. Generated models, test vectors and historical session snapshots may repeat it when their source/revision and non-authoritative role are explicit.” |
| “A rule that duplicates another layer … goes.” | “Remove a safety rule only after showing that it covers no distinct failure mode, timing gap or common-cause failure. Similar checks in host and drive are permitted when their failure coverage differs.” |
| “Code exists only if a test we're actually running needs it.” | “Code exists to satisfy a current milestone requirement, including the safety and recovery behavior required before its hardware test may run.” |
| “Speculative code, ADRs and docs are deleted…” | “Do not build speculative implementations. Preserve decided requirements, unresolved hazards and deferred milestone gates in this plan; remove obsolete instructions after their replacement is available.” |
| “CAN … is the most trusted code, not the most feared.” / “small and readable” | “The CAN boundary has independent protocol vectors, bounded I/O, explicit ownership and tested fault/restart behavior. Its documented dependencies must allow codec review without reading runtime or UI code.” |
| “Every tuned number comes from a logged test…” | “Every tuned value cites a logged experiment. Vendor constants cite a manual revision; initial engineering assumptions are marked unverified and have a named qualification gate.” |
| “Anything we'd do more than twice becomes a script or loop an agent can run and judge on its own.” | “Automate repeated procedures when automation reduces error or effort. A hardware procedure retains its attendance and admission requirements; making it a script does not authorize unattended execution.” |
| “An agent … should know within minutes…” | “README identifies each implemented component’s owner, inputs, outputs, forbidden dependencies and failure behavior, and links the build, vcan and disabled-state deployment procedures.” |
| “One short architecture map…” | “Keep the architecture map in README current whenever a process boundary, CAN owner, configuration authority or stop path changes.” |
| “Each crate or app starts with a few lines…” / “Names say what things do.” | “Each implemented crate or app documents its purpose, prohibited responsibilities and direct dependencies. Names describe those responsibilities.” |
| “about 10 crates in place of 15 crates and 6 bins” | “The table describes responsibilities, not a crate-count target. Create a crate only when an implemented dependency or test boundary warrants it. The proposed table currently lists eight library entries and three binaries, plus UI/MCP clients.” |
| “A wider observability stack choice … is still to be researched.” | “Implement the selected MCAP, candump and Prometheus/Grafana paths first. Add another observability service only to solve a demonstrated missing capability.” |
| “Can be built against the sim now.” | “Implement power-board simulation when preparing its bring-up; preserve its requirements now.” |

**Why:** `PACKET.md` simultaneously requires whole-robot design and build-only-needed scope; `marengo/Cargo.toml` verifies the old 15-library/6-bin count, while `roark/PLAN.md` itself contains the proposed table and premature implementation prompts.

---

**30. Should-fix — Add a factual-status ledger instead of presenting choices as measurements. [Q4]**

**Where:** `roark/PLAN.md`, new “Evidence status” section.

**Current:** No consolidated distinction among verified facts, design choices, provisional values and unverified behavior.

**Add:**

> | Claim used in this plan | Evidence status |
> |---|---|
> | Five limbs, five CAN buses; 23 body joints excluding hands | Target architecture. The joint inventory is supported by `hardware/docs/kinematics.md`; it is not a claim that all joints or buses are built. |
> | Pi 5 control host; Jetson later | Supported by `hardware/STATUS.md`; Jetson integration is deferred work. |
> | Left arm: RS03, RS03, RS02, RS02, RS00; names and IDs 11–15 on can1 | Supported by `hardware/STATUS.md`; elbow/lower-yaw IDs are still identified there as proposals. Verify commissioned identity before enable. |
> | Right arm: IDs 1–5 on can0 | Supported by live `config/motors.yaml`. |
> | Remaining waist/leg bus and ID assignments | Not settled by this plan. The humanoid template conflicts with the live arms and is explicitly provisional. Do not copy it as fact. |
> | Earlier left IDs: 12 on can1; 11–14 on can0 | Historical assignments supported by hardware ADR 0001; current stored IDs require a scan. |
> | Six-reviewer foundation review | Supported by `review/sw-foundation-20261108/RECONCILIATION.md`. Its architectural verdicts are recommendations and its hardware-behavior claims retain their cited evidence limits. |
> | gs_usb, isolated mainline-candleLight F0 adapter; about $60 | Selected direction from `hardware/docs/research/can-stack-for-marengo.md`. Price, availability and exact purchased-board capability are not independently verified by this review. |
> | G431 mainline-firmware incompatibility | A dated finding of the supplied CAN research, not a fresh upstream verification. Verify the actual purchased board/firmware before relying on the label. |
> | SocketCAN 4.0, rtrb, thread-priority, 250 Hz | Chosen stack from the research report, not proof of dependency compatibility or Pi performance. |
> | 60% receive allocation, 300 Hz option, approximately 1 kHz stand operation | Proposed timing targets; hardware acceptance is still required. |
> | 120 Ω ends, approximately 60 Ω combined, 24 V supply without adjustable current limit, 3 A then 10 A fusing | Supported bench-plan values, subject to the harness’s explicit commissioning gates. |
> | RS03 MIT velocity ±20 rad/s | Manual-supported and measured in the cited historical capture; requalify materially different firmware. |
> | RS00 legacy ±50 rad/s / ±17 N·m versus manual ±33 / ±14 | Verified disagreement; installed-drive resolution remains open. |
> | Timeout 0x7028, 20000 counts/s, 1000 = 50 ms | Manual-supported. Actual command-refresh and restart behavior still require qualification. |
> | Torque/current/speed, damper, VBUS and iqf register addresses | Present in the manuals. Mode applicability, ambiguous RS02 table values and installed-firmware behavior remain separate qualification questions. |
> | Three missed replies | Chosen host policy, not a vendor guarantee; must be enforced with elapsed-time checks. |
> | 22–26 V; 60 °C pause; 75 °C stop | Project test limits from CALIBRATION_LOOP.md, not vendor ratings. |
> | 2–3° supervised and ≥5° unattended soft margins | Project minimum margins; stopping-distance sufficiency is unverified. |
> | Elbow 0–124°, 17 N·m design load and approximately 40 N side-load restriction | Supported by hardware plans; design and provisional test values are not proof of completed validation. |
> | RS03/RS02 stall ratings 13/6 N·m | Supported by the manual figures; add RS00 3.6 and RS04 28.5 N·m with test-condition qualifications. |
> | Stand order RS03 waist unit, RS02, RS00, RS04; all four fixture sizes designed | Supported by `hardware/designs/actuator-test-stand/README.md`; designed does not mean printed or accepted. |
> | BRIO owned; camera plan approved; torso, arm and power-board readiness | Dated hardware status, not verified present-day readiness. |
> | Rev B5; heartbeat 50–100 ms; limb shunts 2 mΩ; clamp 30 W/60 s and 3 kJ | Supported by REV_B_PARTS.md. The 30 W cap is provisional pending the heat test; rev B5 does not qualify leg power. |
> | Roark rename and 2026-11-08 decisions | Owner decisions recorded by this handoff, not independently established hardware facts. |

**Why:** This ledger grounds the numerical claims in the files actually read while preserving the difference between a specification, a historical observation and a demonstrated capability.

---

**31. Must-fix — Add only the decisions that genuinely require the owner. [Q7]**

**Where:** `roark/PLAN.md`, new “Owner decisions” section.

**Current:** Owner-dependent choices are scattered or implicit.

**Add:**

> These questions require owner or hardware-owner decisions. Software work continues under the stated defaults; dependent physical operation remains gated.
>
> | Decision needed | Recommended default |
> |---|---|
> | Does milestone 1 mean C0–C2, or the complete unattended C0–C8 programme? | C0–C2 on the assembled arm completes milestone 1; later stages have separate acceptance gates. |
> | Are proposed left IDs 14/15 approved, and what are the final waist/leg addresses? | Retain the left proposal as uncommissioned inventory. Leave unresolved waist/leg addresses unknown; do not assign them from the humanoid template. |
> | Which physical reference pose and support method is approved for each joint? | No position/gravity operation until an attended reference is established. Continue codec, simulation and secured loose-drive work. |
> | What are the actual bench forbidden poses, catch-pad coverage and E-stop power scope? | Admit only attended tests within the verified fixture workspace; no unattended run until these are recorded. |
> | Is loss of one mechanically independent arm allowed to leave another active? | Stop every joint in the affected run; stop all runs for bus-off or shared-control failure. Any broader fault-containment policy needs explicit mechanical independence evidence. |
> | May a finite admitted autonomous stage continue after its UI/agent disconnects? | Yes only while its host runner and every required sensor remain healthy; otherwise stop. No indefinite live-command hold. |
> | Who approves skeleton/model changes and performs legacy deletion in the hardware repository? | Hardware owner approves physical values; software owner reviews schema/implementation; hardware-repository owner deletes legacy software only after milestone 2 evidence is accepted. |
> | What support and power hardware authorize the first leg motion? | No leg enable on rev B5 or without the qualified gantry and leg-power configuration. |
>
> Firmware behavior, cap effectiveness, CAN jitter, current conversion, stopping distance and sensor timing are measurements to obtain, not questions to settle by owner preference.

**Why:** `marengo/hardware/docs/decisions/0001-can-and-motors.md`, `CALIBRATION_LOOP.md`, `sim-to-real-loop.md` and `REV_B_PARTS.md` leave these physical or ownership decisions open; none supplies the missing measurements.

---

**32. Nice-to-have — Make the entry files accurately describe the handoff’s status. [Q1, Q3]**

**Where:** `roark/README.md`, opening paragraph; `roark/CLAUDE.md`, reading order.

**Current:**

> “The control software for Roark … one realtime process … plus a simulated drive, agent tools and an operator UI.”

> “PLAN.md: what to build, in order. Milestone 1 is the left arm online.”

**Replace with:**

> This repository is the handoff for rebuilding Roark’s software. The target is one Pi runtime controlling the fixed robot inventory over five CAN buses, with a vcan drive emulator and clients added as the milestones require. The supplied documents specify intended behavior; they are not evidence that these components already exist.
>
> Read TENETS, then PLAN, then the applicable hardware specifications cited by PLAN. Start with PLAN’s pre-motion acceptance sequence. Record completed gates and unresolved facts in PLAN so the next agent can distinguish implemented behavior from planned work.

**Why:** The supplied `roark/` inventory contains only `CLAUDE.md`, `PLAN.md`, `README.md` and `TENETS.md`; the current present-tense README blurs the target architecture with an implemented system.