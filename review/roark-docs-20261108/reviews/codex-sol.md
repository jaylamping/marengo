**1. MUST-FIX — Establish precedence without allowing the plan to override hardware evidence.** *(Questions 1, 3)*

**(a) Where:** `roark/PLAN.md`, “Start here” and “Rules that hold for every item.”

**(b) Current:**

> “The tenets override every ADR. If a legacy doc and this plan disagree, this plan wins.”

> “Hardware facts change: check STATUS.md before acting on a number here.”

**(c) Replace with:**

> TENETS.md governs software policy; PLAN.md governs implementation order. Neither overrides vendor protocol definitions, hardware release gates, or measured limits.
>
> Use the reference repository’s `hardware/STATUS.md` as the index to hardware evidence. STATUS.md explicitly gives its linked detailed documents precedence. When sources disagree, record the conflicting values and their provenance; do not select the more permissive value. Leave the affected capability unavailable until the conflict is resolved.
>
> Legacy architecture decisions do not prescribe the new architecture. Legacy measurements remain evidence, with their original hardware, firmware and transport qualifications.

**(d) Why:** `marengo/hardware/STATUS.md` explicitly defers to linked documents; the present wording would let software policy overrule physical constraints.

---

**2. MUST-FIX — Define the skeleton’s required content and validation.** *(Questions 1–3)*

**(a) Where:** `roark/PLAN.md`, §1, after the opening paragraph.

**(b) Current:**

> “One file, `config/skeleton.yaml`, describes the whole robot…”

> “Unbuilt limbs are listed and simply show offline.”

> “Directions for the left joints are unknown until the sign check; leave them as placeholders.”

**(c) Add:**

> The skeleton schema must define:
>
> - Schema version and robot display name.
> - The five limb memberships and stable joint order.
> - Each joint’s actuator model, logical bus, device ID, expected MCU identity when commissioned, and qualified firmware/protocol profile.
> - Direction, zero convention and reference validity requirements.
> - Hard travel bounds, supervised and unattended soft bounds, and approved torque, current and speed ceilings.
> - The source and verification status of each hardware value.
> - Stable physical adapter identification for each logical bus.
>
> Angles are output-joint radians, angular velocities radians per second, torques output-joint N·m, temperatures °C, voltages V and durations explicitly identified as seconds or milliseconds. Current values must identify whether they mean drive phase current, Iq or DC supply current; these are not interchangeable.
>
> Unknown values are explicitly unresolved, never plausible numeric defaults. Unbuilt or unresolved joints remain visible in inventory but cannot be enabled. Discovery does not make a joint commissioned.
>
> Reject duplicate joint names, duplicate `(bus, device ID)` addresses, invalid memberships, unsupported models, non-finite values, reversed bounds, soft bounds outside hard bounds, and caps outside the qualified model/firmware limits. Missing data for an unselected joint must not prevent an independent, explicitly selected bench test.
>
> Completion: the loader accepts the complete inventory, rejects each invalid case above, and reports every reason a selected joint cannot enable. No second hardware configuration file supplies fallback values.

**(d) Why:** `marengo/config/motors_humanoid.yaml` contains explicitly provisional mappings, while `marengo/hardware/STATUS.md` corrects its arm mappings; numeric placeholders must not become enable permission.

---

**3. MUST-FIX — Specify zero, sign and power-cycle semantics.** *(Questions 1, 2, 4)*

**(a) Where:** `roark/PLAN.md`, §1; add a “Coordinates and reference” subsection.

**(b) Current:**

> “…direction, zero, soft and hard limits…”

**(c) Add:**

> Let `p` be the drive-reported output angle, `z` the drive-coordinate angle corresponding to the agreed physical joint zero, and `s` the verified direction, either +1 or −1. Joint coordinates are `q = s × (p − z)`. Joint velocity and torque use the same direction transformation; commanded position uses the inverse transformation. Do not apply the actuator’s internal reduction ratio again.
>
> Define angle wrapping and continuity explicitly. A startup angle must not be unwrapped into a permitted position merely because that branch fits the limits.
>
> Host offset calibration and the drive’s Set Zero command are different operations. Normal calibration changes the proposed host offset; it does not silently issue Set Zero or write `zero_sta` or `add_offset`.
>
> A reference is valid only for the current runtime session and verified drive continuity. Runtime restart, lost drive continuity, unexpected Reset mode, identity change or drive power cycle invalidates it. A matching MCU identity alone does not prove that the drive retained its zero.
>
> Before reference-dependent motion resumes, restore the physical reference, complete parameter preflight, and verify the reference readback. No automatic re-zero, re-enable or trajectory continuation is allowed.
>
> Completion: round-trip coordinate tests cover both directions, nonzero offsets and wrap boundaries; the simulator proves that a reboot with the same MCU identity cannot retain motion permission.

**(d) Why:** `marengo/crates/davout/tests/physical_firmware/mod.rs` models reboot loss of zero, and `marengo/crates/robstride/src/params.rs` distinguishes the firmware zero registers from host coordinates.

---

**4. MUST-FIX — Replace discovery-only readiness with a small explicit state machine.** *(Questions 1–3)*

**(a) Where:** `roark/TENETS.md`, §1; `roark/PLAN.md`, §6.

**(b) Current:**

> “When a drive answers it is registered and addressed; when it doesn't, it shows as offline. Nothing else.”

> “No mix-and-match of actuator types per limb…”

**(c) Replace with:**

> Actuator assignment is fixed per joint. An arm may contain several actuator models; discovery never substitutes one model or identity for another.
>
> Track communication presence separately from permission to move. The minimum drive states are Offline, Disabled/Unverified, Preflight, Ready, Enabling, Running, Stopping and FaultLatched.
>
> Ready requires a qualified identity and protocol profile, valid reference where needed, verified direction, fresh required telemetry, matching drive settings and no latched fault. Only an explicit enable request enters Enabling. Running requires observed Run feedback within the qualified enable deadline.
>
> Any failed transition returns to disabled or latched-fault handling; it never proceeds optimistically. A returning drive enters Disabled/Unverified.
>
> A test names its exact joint set. Unbuilt joints outside that set do not block it. Every selected joint must pass admission. A limb reports both its available inventory and the state of its selected energized joints; it must not report “ready” merely because one joint answers.
>
> Completion: transition tests cover partial inventory, enable failure halfway through a group, unexpected Reset, identity change, silence and reconnection.

**(d) Why:** `marengo/hardware/STATUS.md` assigns RS03, RS02 and RS00 within one arm; `marengo/review/sw-foundation-20261108/RECONCILIATION.md` identifies offline-joint admission as a legacy blocker.

---

**5. MUST-FIX — Stop claiming that parameter readback proves mode-independent protection.** *(Questions 2, 4)*

**(a) Where:** `roark/TENETS.md`, §3, drive layer; `roark/PLAN.md`, §6, torque/current/speed row.

**(b) Current:**

> “Drives (holds even if the Pi crashes): CAN host-loss timeout of 50 ms or less, torque, current and speed caps…”

> “Per joint from the skeleton; never above the weakest printed part on that joint”

**(c) Replace with:**

> For every control mode used, qualify which drive-side limits actually constrain that mode on the installed firmware. The manuals describe `limit_spd` as a CSP position-mode speed limit and `limit_cur` as a position/velocity-mode current limit. Do not assume either limits operation-control/MIT mode.
>
> Before enabling a mode, write and read back its applicable limits and retain the measured evidence that they constrain output in that mode. A successful write or matching readback proves configuration, not enforcement.
>
> The host must bound total MIT torque:
>
> `τ = Kp × (q_target − q) + Kd × (v_target − v) + τ_feedforward`.
>
> Bounding feed-forward alone is insufficient. Account for feedback age, quantization and motion between feedback and command application. If the resulting bound cannot be established within the approved joint cap, refuse the command or disable. Do not label a prediction a guaranteed physical torque bound.
>
> Each approved cap is no greater than the applicable actuator, printed-part, fixture and electrical limits. Record current-unit conversions and the evidence supporting them.
>
> Completion: low-load, supported tests verify each required limit in each enabled mode; unsupported protection remains an enable blocker.

**(d) Why:** The manuals’ §4.1.13 parameter tables are mode-specific, and `marengo/crates/davout/src/total_torque.rs` explicitly says total MIT clamping was undocumented and unverified.

---

**6. MUST-FIX — Correct the timeout predicate and remove the ambiguous reset-report assumption.** *(Questions 1, 2, 4)*

**(a) Where:** `roark/PLAN.md`, §6, `canTimeout` row.

**(b) Current:**

> “Write 1000 (= 50 ms; 20000 = 1 s) and read back before every enable; refuse on 0 or above. Re-check after any drive power cycle or reset report”

**(c) Replace with:**

> Write `canTimeout` 0x7028 as uint32 value 1000 and require a successful, fresh readback of 1000 before every enable. Zero disables timeout protection and is forbidden. Any alternative timeout must be explicitly approved, positive, no greater than 1000, and compared against its intended value.
>
> Treat an observed or suspected drive reset, power interruption or loss of continuity as invalidating the entire preflight and reference, not just this register. Do not depend on a dedicated reboot notification: none is established by this packet.
>
> Qualify which received frame types refresh the timeout. After an unsafe loss of control, parameter polling or stale command retransmission must not keep an otherwise uncontrolled drive energized.
>
> Completion: the simulator and a supported drive test demonstrate timeout-to-Reset, including a crashed runtime and a surviving bus thread with no fresh control command.

**(d) Why:** All four local manuals, §3.3.6 and the parameter tables, establish 20000 units per second and Reset on timeout; “above” currently has no comparison value.

---

**7. MUST-FIX — Make enable a bounded transaction.** *(Questions 1, 2, 5)*

**(a) Where:** `roark/PLAN.md`, §6, new “Enable sequence.”

**(b) Current:** No complete sequence; §2 only names “enable and stop order.”

**(c) Add:**

> Enable is one runtime-owned transaction:
>
> 1. Reject concurrent motion, commissioning and parameter-changing requests for the selected group.
> 2. Establish disabled state and drain stale receive data without blocking indefinitely.
> 3. Verify selected addresses, MCU identities, qualified firmware profiles and reference validity.
> 4. Send active-reporting Off and observe its local echo; retain the measured settling interval before Enable.
> 5. Write and freshly read back timeout and every applicable cap; read damper, VBUS, temperature and fault state. Any failed, stale, mismatched or non-finite result aborts.
> 6. Establish safe initial targets and the qualified mode-specific ordering. The legacy sequence is Enable followed by RunMode; the vendor instructions for current, velocity and position modes put RunMode before Enable. Preserve the qualified sequence for each mode rather than declaring one universal order.
> 7. Require fresh Run feedback before admitting motion. The enabling deadline must accommodate the measured firmware response and remain bounded.
> 8. If any selected drive fails, disable every drive already armed by this transaction and latch the failure.
>
> Set Zero is a separate attended reference operation. Preserve the measured 800 ms post-SetZero quiet interval before subsequent enable; 450 ms is the beginning of a guarded blackout window, not the blackout’s duration.
>
> Completion: tests cover missing readbacks, stale acknowledgements, unexpected reports, partial group enable and reset during each phase.

**(d) Why:** `marengo/docs/commissioning/firmware/robstride-firmware-behavior.md` documents the quiet intervals and enable timing; the local manuals’ §4.3 instructions differ from the legacy ordering.

---

**8. MUST-FIX — Define stop actions and report what was actually achieved.** *(Questions 1, 2)*

**(a) Where:** `roark/PLAN.md`, §6; replace the silence/fault/bus-off action descriptions and add stop semantics.

**(b) Current:**

> “Stop the limb after 3 missed replies…”

> “Stop on any fault bit; never auto-clear and retry”

> “Stop the whole robot; no automatic restart while moving”

**(c) Replace with:**

> A protective stop first prevents further motion commands and cancels queued work. Faults, stale drive feedback, unexpected Reset, bus loss, invalid control state and uncertain stop delivery take the immediate-disable path. They never request a move to a rest pose.
>
> On every reachable affected bus, send Disable without fault clearing, followed by active-reporting Off. Do not wait for an acknowledgement from one drive before attempting the others. Missing buses and failed writes must not prevent attempts on healthy buses.
>
> Retain the legacy SpeedTarget-zero → neutral-MIT → Disable sequence only where the selected mode’s qualification requires it. Those preliminary writes must not postpone emergency Disable.
>
> A normal requested stop may move to a validated rest pose only while every required drive, sensor and control deadline remains healthy. Any fault during that move switches immediately to disable.
>
> Report separately: stop requested; frames accepted by the kernel; Reset observed; and delivery unconfirmed. Socket acceptance is not proof of drive disable or physical standstill. A disabled arm can still fall.
>
> Bus-off stops all energized robot motion. Until narrower fault containment is demonstrated on the supported robot, use the same robot-wide scope for other protective stops.
>
> Completion: test one lost bus while another is moving, a full TX queue, a nonresponsive drive and a slow logger. Stop attempts on healthy buses must complete without waiting for the failed component.

**(d) Why:** `marengo/bins/motor-repl/src/stop.rs` distinguishes accepted writes from confirmed drive state; `marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md` §9 requires immediate disable when anything is unhealthy.

---

**9. MUST-FIX — Cover a dead control thread while CAN threads remain alive.** *(Questions 1, 2)*

**(a) Where:** `roark/PLAN.md`, §§3 and 6, host-hang rule.

**(b) Current:**

> “systemd `WatchdogSec` fed from the control thread; later the power board heartbeat”

**(c) Replace with:**

> Each control publication has a monotonic generation and expiry. A bus thread may transmit only commands from a current publication. It must not repeatedly transmit the last nonzero command after the control thread stops publishing.
>
> Expired control data or a stalled required bus thread latches a protective stop. Stop feeding the systemd watchdog and, when fitted, the power-board heartbeat. Watchdog feeding requires progress of the control loop and every bus required by the active motion.
>
> Configure and measure the systemd watchdog and termination behavior; their timing is not yet specified by this packet. Drive timeout remains the independent backstop while systemd detects and terminates a hung process.
>
> Runtime startup and service restart begin disabled, discard previous motion requests and require fresh preflight and explicit enable.
>
> Completion: separately suspend the control thread, a bus thread and a non-realtime worker; terminate the runtime; and power-cycle the Pi. Verify the specified stop behavior in each case.

**(d) Why:** `marengo/hardware/electrical/pdb/REV_B_PARTS.md` requires heartbeat progress from the control loop itself; continued stale CAN traffic could otherwise defeat the drive timeout.

---

**10. MUST-FIX — Define receive validity and liveness under delayed or misleading traffic.** *(Questions 1, 2)*

**(a) Where:** `roark/PLAN.md`, §3, after the tick description.

**(b) Current:**

> “…collect replies until about 60 % of the period, publish a snapshot with each joint's age.”

**(c) Add:**

> Accept drive state only from a valid extended data frame with the required length, expected bus, source address, destination/host convention and message type. Local TX echoes, identity replies, version replies and parameter replies are not pose feedback.
>
> Match parameter responses to the requested bus, drive, index, successful status and current transaction. Reject malformed and non-finite decoded values.
>
> Maintain monotonic last-valid-feedback age independently of the tick miss counter. Three consecutive missed reply deadlines trigger a stop; skipped control ticks and old queued frames must not extend that deadline. Count at most one accepted liveness update per drive per collection interval.
>
> The private protocol establishes no command sequence counter in this packet. Do not claim exact command/reply correlation or general duplicate detection from identical payloads.
>
> Receive draining is bounded by both time and work. RX overflow, socket failure, adapter removal, bus-off and incomplete safety-critical reception invalidate state and trigger protective handling. Unexpected Reset while running is a fault.
>
> Completion: inject wrong-host frames, version-as-feedback, delayed old replies, duplicates, malformed lengths, receive flooding and overflow.

**(d) Why:** `marengo/crates/robstride/src/identity.rs`, `params.rs`, `bus.rs` and `marengo/docs/commissioning/firmware/robstride-firmware-behavior.md` expose these distinct frame classes and stale-reply hazards.

---

**11. MUST-FIX — Resolve scan/set-ID ownership and the forbidden-type conflict.** *(Questions 2, 3)*

**(a) Where:** `roark/PLAN.md`, §3 and the `robot-cli` layout row.

**(b) Current:**

> “Only the realtime process transmits. The one exception is the independent `disable` run by systemd after the process exits.”

> “Tools it needs: a drive scan … and a one-drive-at-a-time set-ID.”

**(c) Replace with:**

> Scan, parameter inspection and set-ID are requests to the runtime; the CLI never opens a transmitting socket for them.
>
> Set-ID is available only in exclusive commissioning state, with motion disabled and exactly one physical drive connected to the selected bus. Read its MCU identity, perform the explicitly requested type-7 change, then verify the same identity at the new address. This changes drive configuration, not the committed skeleton.
>
> Type 7 is forbidden on the normal motion path and allowed only through this commissioning operation. Parameter save, baud change and protocol switching remain forbidden unless separately required and qualified. Writes to `zero_sta` and `add_offset` remain prohibited.
>
> The independent disable helper is transmit-only and runs after the runtime is confirmed exited. Service ordering must prevent overlap with a replacement runtime. It uses the last successfully validated address set and has no dependency on dynamics, logging or a complete motion preflight.
>
> Completion: concurrent commissioning/motion is refused; duplicate runtime startup fails; helper failure on one interface does not skip another.

**(d) Why:** `marengo/crates/robstride/src/bus.rs` forbids type 7, while `marengo/review/sw-foundation-20261108/RECONCILIATION.md` requires a new set-ID facility and a single CAN owner.

---

**12. MUST-FIX — Make electrical readiness a gate, not a calendar statement.** *(Questions 1, 4, 5)*

**(a) Where:** `roark/PLAN.md`, “When the hardware is ready,” first row; §5.

**(b) Current:**

> “Ready now (drives owned, bench harness planned)”

> “Scan, set IDs one drive at a time, then a CAN soak…”

**(c) Replace with:**

> Drives and supply are available; powered testing is not ready until the bench harness, physical E-stop, fusing, termination, grounding and actuator restraint have been checked.
>
> Before first power, record continuity, absence of a supply short, connector pin checks and approximately 60 Ω across CAN_H/CAN_L with power off. Adapt the harness’s HAT-specific termination and ground instructions to the actual isolated USB adapter; do not defeat isolation by copying an unexamined ground tie.
>
> First power uses the documented 3 A trunk fuse and one restrained drive. Increase to 10 A only after the documented individual checks pass.
>
> After address commissioning, run the harness’s 10-minute polling soak. Pass requires no error frames or RX overflow and unchanged zero TX/RX error counters. Motion remains disabled until safety preflight, supported sign/reference checks and stop tests pass.
>
> Printed-arm motion additionally requires the actual print revision, fastener stacks, connector fit and unpowered harness sweep to be released for that test.

**(d) Why:** `marengo/hardware/electrical/wiring/left-shoulder-bench-harness.md` is a provisional bench plan and explicitly lists these prerequisites.

---

**13. MUST-FIX — Separate the bench voltage policy from the vendor operating range and regeneration handling.** *(Questions 2–4)*

**(a) Where:** `roark/PLAN.md`, §6, VBUS row; “Rules that hold for every item.”

**(b) Current:**

> “22 to 26 V on the bench supply; no faults present”

> “The bench supply has no current limit. 3 A fuse for first power-up, 10 A after.”

**(c) Replace with:**

> The calibration specification currently requests a 22–26 V bench stop window, while the local actuator manuals specify a 24–60 V operating range. These are different statements. Do not qualify operation below 24 V from the calibration window alone; resolve the lower operating threshold before powered acceptance.
>
> The fixed bench supply has no adjustable current limit. Fuses protect wiring; they do not enforce commanded actuator torque or absorb regeneration.
>
> Before powered trajectories, establish a measured braking-energy envelope for this supply and load. VBUS is polled in bounded slack with a defined freshness deadline; stale voltage data blocks enable or stops an active run.
>
> A voltage excursion takes the immediate protective path. Do not respond to an overvoltage condition by commanding an unqualified regenerative return to rest.
>
> Bench and battery operation have distinct voltage, startup and energy rules; selecting the power source must select the corresponding validated rules.

**(d) Why:** `marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md` §9 specifies 22–26 V; the four manuals specify 24–60 V, and the harness’s “Supply notes” identifies regeneration into a nonabsorbing supply.

---

**14. MUST-FIX — Resolve the stalled-test prohibition and fill in the remaining stall ratings.** *(Questions 1, 3, 4)*

**(a) Where:** `roark/PLAN.md`, §7; §8 “Never”; “Rules that hold for every item.”

**(b) Current:**

> “Limits: stalled holds stop at the stall rating (RS03 13 N·m, RS02 6 N·m; check RS00 and RS04)…”

> “Never … hold a stall…”

> “Nothing overloads, overheats or stalls a real drive.”

**(c) Replace with:**

> Unintended stalls during arm motion stop the run. The only planned stationary-load exception is the attended actuator-stand procedure for T2, T3 and T5, with the fixture restrained, bounded loading, valid temperature telemetry and the hardware E-stop available.
>
> The local vendor stationary-load tables mark these rated stalled loads: RS00 5 N·m, RS02 6 N·m, RS03 13 N·m and RS04 28.5 N·m. These ratings depend on the vendor test conditions and do not qualify the printed stand’s thermal installation.
>
> T2 and T5 use the approved stationary-load ceiling, not the rotating or peak torque rating. T5 stops at 60 °C or on any earlier safety trigger.
>
> The drive cap is the test maximum plus 10% only when that remains below every approved actuator, fixture, structure and electrical ceiling. Otherwise reduce the test maximum; do not raise the protective ceiling.
>
> Record the approved load duration, cooling conditions and stop behavior in the test plan before execution. No test deliberately reaches the vendor thermal protection threshold.

**(d) Why:** The actual stall-table images establish the missing values and distinguish them from rotating ratings:

- :codex-file-citation{path="C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/robstride/RS00_User_Manual_260713.pdf" purpose="source"}
- :codex-file-citation{path="C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/robstride/RS02_User_Manual_260713.pdf" purpose="source"}
- :codex-file-citation{path="C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/robstride/RS03_User_Manual_261013.pdf" purpose="source"}
- :codex-file-citation{path="C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/robstride/RS04_User_Manual_260713.pdf" purpose="source"}

---

**15. MUST-FIX — Do not turn the elbow’s structural design load into its operating torque.** *(Questions 1, 4)*

**(a) Where:** `roark/PLAN.md`, §8, elbow torque row.

**(b) Current:**

> “≤ 17 N·m (printed stop design load), lower if the rig fails low”

**(c) Replace with:**

> The printed elbow stop’s design load is 17 N·m; this is not permission for a powered joint to apply 17 N·m continuously or strike the stop.
>
> The operating cap is the lowest applicable approved actuator, structural, thermal, electrical and stage limit. The dummy rig must pass its agreed release criteria before forearm commissioning. A failed structural gate requires hardware review and retest; reducing a software cap does not automatically release a failed assembly.
>
> The nominal elbow hard stops are 0° and 124°. Supervised and unattended soft bounds must use the actual released geometry and the specified inset. Do not seek hard stops under power to teach them.

**(d) Why:** `marengo/hardware/designs/left-arm/testing/ELBOW_DUMMY_OUTPUT_TEST.md` separates design load, proposed failure criteria and release actions; `CALIBRATION_LOOP.md` §14 uses rated-torque-derived operating loads.

---

**16. MUST-FIX — Restore the missing model contract without making URDF another limit authority.** *(Questions 1–3)*

**(a) Where:** `roark/TENETS.md`, §2; `roark/PLAN.md`, §1.

**(b) Current:**

> “The runtime reads that file and nothing else about the hardware.”

> “URDF and MJCF are for sim and dynamics only.”

**(c) Replace with:**

> The skeleton is the sole runtime authority for inventory, addressing, coordinate conversion and safety limits. The runtime never reads CAD.
>
> Gravity compensation, kinematics and collision checks may consume a versioned derived dynamics model. That model supplies geometry, inertias and collision shapes; it does not override skeleton IDs, signs or limits. The skeleton identifies the approved model revision.
>
> Unknown or provisional model data must be reported. Torque-only loose-drive commissioning must not require a full-body dynamics model. Gravity compensation and collision-dependent stages require an approved model for their selected joints and attached payload.
>
> Completion: conflicting model limits cannot change runtime limits; a missing dynamics model blocks only capabilities that need it; logs identify the exact model used.

**(d) Why:** `marengo/hardware/designs/left-arm/urdf/README.md` identifies provisional geometry and masses, while `marengo/hardware/docs/kinematics.md` documents known right-arm axis and fitted-COM problems.

---

**17. MUST-FIX — Make camera safety and autonomous authority part of the handoff.** *(Questions 1–3, 5)*

**(a) Where:** `roark/PLAN.md`, §9.

**(b) Current:**

> “Order inside it: camera service and stage C0 … then … C1 and C2 attended with Joey. Reuses item 6's preflight.”

**(c) Replace with:**

> Implement the camera service and C0 before powered calibration. Record and read back the fixed BRIO settings and pass the source specification’s C0 gates before trusting camera measurements.
>
> The host executes a submitted stage plan; an agent never streams live calibration commands. The plan identifies joints, trajectories, payload, rest poses, caps, required sensors, code revision, skeleton revision and model revision.
>
> Carry all §9 stop rules from the calibration specification into the runtime: camera freshness and required-tag visibility, residual jumps, unexpected hold torque, temperatures, voltage, limits, drive faults and drive silence.
>
> Camera staleness or missing tags stop trajectory progression immediately. Holding is permitted only while drive feedback and torque/thermal limits remain valid; otherwise disable. Never continue the trajectory blind.
>
> C1 and C2 start attended. Any unattended stage must first pass attended with the same code, configuration and hardware. C5 remains attended. The stage runner cannot change limits, flash firmware, power-cycle drives or issue a new firmware zero outside the reference workflow.
>
> The source specification permits bounded autonomous calibration corrections, but committed hardware facts remain hardware-owned. Until that ownership conflict is resolved, agents produce proposed corrections and a replay/no-refit report; the runtime does not apply them to the active skeleton.
>
> Completion: each implemented stage produces its specified gate table, raw evidence and pass/fail outcome. A failed gate cannot advance the circuit.

**(d) Why:** `marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md` §§8–11 contains these rules, including the correction authority that currently conflicts with TENETS §2.

---

**18. MUST-FIX — Expand power-board integration beyond a list of feature names.** *(Questions 1, 2, 4, 5)*

**(a) Where:** `roark/PLAN.md`, §10.

**(b) Current:**

> “…heartbeat … port-fault reporting, INA237 meters … self-test, brake clamp caps … brown-out budget, E-stop input debounce. Can be built against the sim now.”

**(c) Replace with:**

> Implement and qualify the complete rev B5 software requirements before battery-powered enable:
>
> - Toggle WD_FEED from demonstrated control-loop progress every 50–100 ms. Verify the hardware drops and latches motor power off when heartbeat stops; the specified watchdog interval is 270–355 ms.
> - Read ESTOP_PI_N as information only, with no GPIO pull and invalid state until configured. Software must not participate in or delay the hardware E-stop path.
> - Decode PORT_FLT_N using the meter topology. The right-arm meter reads the port input, so identifying that port is not the same as finding a low output meter.
> - Use the INA237 register map and documented addresses. Limb and 12 V shunts are 2 mΩ; SPARE_SW is 5 mΩ with ADCRANGE 1. Integrate energy in the host.
> - Refuse enable while BMS telemetry indicates charging or is stale. Define the enable freshness deadline before battery commissioning.
> - Apply the measured pack-resistance current budget and the documented aggregate electrical limits. Drive Iq/phase-current settings must not be summed as though they were measured battery current.
> - Shut compute down at minimum cell ≤3.0 V, filtered pack ≤38 V or BMS telemetry loss >5 s, with motor shutdown handled first.
> - Run the specified self-test only with banks open, VBUS <3 V, at least 5 s after the last stop and pack ≥40 V. Apply the calibrated MID divider correction and the source document’s acceptance windows.
> - With the battery power path, do not load drives until bus voltage exceeds 45 V. This condition does not apply to the separate 24 V bench power path.
> - Enforce braking restrictions when the pack is nearly full or the BMS refuses charge. Compute clamp power as `duty × VBUS² / 10 Ω`, then integrate it to energy. Enforce the rolling 60 s average cap and 3 kJ burst cap. Define burst completion/reset from thermal evidence before permitting repeated bursts.
> - At an energy limit, use a previously qualified non-regenerative protective action; do not assume “return to rest” is safe.
>
> Completion: simulator tests cover stale BMS/meters, charge state, port trip, watchdog loss and threshold crossings; hardware acceptance covers the specified self-test, watchdog, E-stop latch and power measurements.
>
> Rev B leaves leg ports unfitted and is not the released full-body power solution. Milestone 5 requires the leg-capable power revision and its updated limits before powered leg acceptance.

**(d) Why:** `marengo/hardware/electrical/pdb/REV_B_PARTS.md`, “Accepted risks and software requirements,” specifies these requirements and explicitly reserves legs for rev C.

---

**19. MUST-FIX — Keep logging failure out of the realtime path while defining what happens to a logged experiment.** *(Questions 1–3)*

**(a) Where:** `roark/TENETS.md`, §5 final paragraph and §8; `roark/PLAN.md`, “Observability.”

**(b) Current:**

> “Everything else (UI, logging, storage, agent tools) … can crash without affecting motion.”

> “Every frame in and out goes to MCAP from the realtime process … with kernel timestamps…”

**(c) Replace with:**

> UI, storage, metrics export and agent clients cannot block the control or CAN threads. Realtime threads publish bounded records to a non-realtime recorder; they do not perform file I/O, compression, network export or unbounded allocation.
>
> Recorder failure, a full queue, disk-full and disk-stall conditions have explicit outcomes. Recorders report dropped-record counts and mark the session incomplete. A calibration or measurement run that requires complete evidence stops through the normal protective mechanism when recording continuity is lost.
>
> Loss of an observational client does not itself stop a valid host-owned plan. Loss of a sensor or active command source required by that plan does.
>
> Disable precedes log flushing during shutdown. Independent candump improves evidence recovery but is not guaranteed lossless or immune to disk failure.

**(d) Why:** `marengo/review/sw-foundation-20261108/RECONCILIATION.md` identifies flush-before-disable as a blocker; the CAN research specifies a bounded realtime-to-logger ring.

---

**20. MUST-FIX — Define the one runtime interface and motion authority.** *(Questions 1, 2)*

**(a) Where:** `roark/PLAN.md`, after §3; `api` layout row.

**(b) Current:**

> “The one interface to the realtime process”

**(c) Add:**

> The initial API is a versioned local interface on the Pi. Use a Unix-domain socket with length-delimited JSON messages outside realtime threads. UI and MCP bridges use this same interface and do not expose raw CAN writes.
>
> The initial message set covers inventory/state, preflight and its failures, commissioning scan/set-ID, attended reference/sign checks, explicit enable, disable/stop, submission/cancellation of host-owned stage plans, run status and result retrieval.
>
> Read access does not grant motion authority. Only one accepted motion operation controls a selected joint group at a time; conflicting requests are refused. Motion commands cannot implicitly enable a drive.
>
> Requests carry an identity and expiry. Retrying a request must return its existing outcome rather than execute motion twice. Expired, superseded and pre-restart requests cannot execute.
>
> Stop has priority over queued work and is idempotent. A lost client connection does not cancel an already accepted host-owned stage solely because the connection disappeared; any operation requiring continuing client input stops when that input expires.
>
> Completion: disconnect, reconnect, retry, conflicting-command and runtime-restart tests prove these semantics.

**(d) Why:** This is a proposed interface choice, not an existing implementation; `marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md` §10 requires host execution, and the reconciliation identifies multiple ingress paths as a legacy problem.

---

**21. MUST-FIX — Give each milestone an explicit acceptance boundary.** *(Questions 1, 2, 5)*

**(a) Where:** `roark/PLAN.md`, “Milestones.”

**(b) Current:**

> “Each milestone is ‘these drives come online and pass their tests’. None of them adds a code path of its own.”

**(c) Replace with:**

> A milestone passes only when its required hardware is released, its selected joints pass commissioning and safety acceptance, its required tests have complete logs, and no unresolved failure affects those capabilities. Hardware-specific values are data; different fixtures and sensor-dependent tests may require different test procedures.
>
> | Milestone | Acceptance |
> |---|---|
> | 1 — Left arm | Loose-drive identity, addressing, timeout/cap qualification, sign/reference and stop tests pass; the stand tooling passes on the available approved unit without waiting for every future actuator type; released printed-arm geometry passes the required calibration stages. Record stage-by-stage completion rather than calling C1/C2 alone the complete camera loop. |
> | 2 — Right-arm migration | Right arm passes the new admission and stop rules; both arms operate concurrently from one skeleton on their separate buses; loss of either bus stops the required scope; legacy evidence and tools are preserved before deletion. |
> | 3 — Waist | Released waist mounting and reference procedure pass; three-bus concurrency and failure handling pass. Do not reuse the stand reference as an installed-waist reference. |
> | 4 — Board and battery | All §10 interlocks and hardware acceptance gates pass before battery-powered motion. |
> | 5 — Legs | Leg-capable power hardware, released leg mappings/geometry and support gantry are available; all five buses pass load and failure tests; first powered leg tests remain supported. |
>
> A software item is complete when its defined behavior is implemented, its named failure cases pass and its evidence can be replayed or inspected. “Proves the whole stack” does not claim qualification of hardware not yet fitted.

**(d) Why:** `marengo/hardware/docs/sim-to-real-loop.md` allows actuator-type tests as units become available; `CALIBRATION_LOOP.md` defines C0–C8 separately; `REV_B_PARTS.md` excludes legs from rev B.

---

**22. MUST-FIX — Make right-arm migration a requalification, not a copy.** *(Questions 2–4)*

**(a) Where:** `roark/PLAN.md`, “Before the old stack is deleted.”

**(b) Current:**

> “Move the right arm into the skeleton as a limb on `can0`…”

**(c) Replace with:**

> The right arm is present in the skeleton from the initial inventory; milestone 2 commissions it on the new runtime.
>
> Lift the live `can0` IDs 1–5, measured directions, taught bounds, firmware records and friction evidence from the live configuration, retaining their provenance. Do not copy humanoid-template bus mappings or treat legacy soft bounds as satisfying the new required inset.
>
> Reconcile the known lower-arm axis error and the fitted-COM provenance before enabling model-dependent right-arm control. Qualify the RS00 protocol scales for the new stack before either arm’s RS00 is powered by it.
>
> Repeat preflight, reference, stop, power-cycle and simultaneous two-bus acceptance. A previously working legacy arm is not grandfathered past the new safety requirements.
>
> Preserve `gravity-fit`, `firmware-timing`, commissioning procedures and the evidence they consume in usable form before removing their legacy implementation.

**(d) Why:** `marengo/config/motors.yaml`, `control.yaml` and `marengo/hardware/docs/kinematics.md` distinguish measured data from provisional limits and identify the axis/COM issues.

---

**23. MUST-FIX — Resolve deletion timing and cross-repository ownership.** *(Questions 1, 3)*

**(a) Where:** `roark/TENETS.md`, introduction; `roark/PLAN.md`, layout introduction and deletion section; `roark/CLAUDE.md`, reference-repository rule.

**(b) Current:**

> “…once the left arm passes its tests, the right-arm-era code that wasn't carried over is deleted.”

> “…they're deleted when their replacement passes…”

> “The reference repo … is read-only from here.”

**(c) Replace the deletion wording with:**

> No legacy software is deleted before milestone 2 passes and the evidence-preservation checklist is complete.
>
> The Roark implementation agents treat Marengo as read-only. At milestone 2 they produce the exact legacy deletion list and preserved-artifact locations. Deletion in Marengo is performed by its owning project in a separate change; no Roark agent silently crosses that repository boundary.
>
> Remove stale current architecture and setup instructions when replacements land. Preserve dated historical measurements and commissioning records, clearly marked as historical.

**(d) Why:** `PACKET.md` fixes deletion after milestone 2; the current TENETS timing conflicts with PLAN and CLAUDE’s repository boundary.

---

**24. SHOULD-FIX — Correct the startup order.** *(Questions 1, 5, 6)*

**(a) Where:** `roark/PLAN.md`, “Start here.”

**(b) Current:**

> “Set up the new crate layout … and write the skeleton YAML…”

> “Lift the hard-won knowledge…”

> “Build the new CAN layer and its simulated drive together…”

> “Then the actuator test stand tooling … and the camera loop…”

**(c) Replace the numbered list with:**

> 1. Read TENETS, PLAN, hardware STATUS and the linked commissioning/safety sources. Record unresolved hardware facts before assigning operational values.
> 2. Lift protocol vectors, firmware timing, right-arm evidence and stop behavior before designing their replacements.
> 3. Define the skeleton and coordinate/reference contract. Include the complete inventory; unresolved joints remain non-enableable.
> 4. Create only the packages needed for the loader, codec and first executable path. Establish the pinned build and Linux CI.
> 5. Build the codec, deterministic protocol simulator, runtime state machine, safety preflight, API, bounded logging and process-exit handling together on vcan.
> 6. Pass the fault, stale-command, crash, restart and power-cycle simulations before powered motion.
> 7. Qualify one USB adapter and the real bench harness. Measure transport and loop behavior on the Pi with the recorder, candump and intended camera workload.
> 8. Commission restrained loose drives. Implement and run stand tests as released fixtures and units become available.
> 9. Release the installed geometry and payload, pass unpowered camera C0, then proceed through attended and subsequently permitted unattended calibration stages.
>
> Safety and logging are prerequisites to bring-up, not later items implied by their section numbers.

**(d) Why:** The existing list skips explicit safety implementation before hardware; `marengo/review/sw-foundation-20261108/RECONCILIATION.md` lists pre-motion blockers.

---

**25. SHOULD-FIX — Turn CAN timing suggestions into measured acceptance criteria.** *(Questions 1, 2, 4)*

**(a) Where:** `roark/PLAN.md`, §3 and §5.

**(b) Current:**

> “…until about 60 % of the period…”

> “250 Hz default; 300 Hz only if measured.”

> “PREEMPT_RT only if the Pi 5 measurement misses its budget.”

**(c) Replace with:**

> Use 250 Hz initially: a calculated 4 ms control period. Treat the research’s 60% receive window as an initial scheduling target, not a demonstrated worst-case deadline.
>
> Before physical acceptance, record the chosen send, receive, control-publication and command-expiry deadlines and explain how they remain inside the drive timeout. Parameter traffic uses a bounded slack budget and cannot delay cyclic commands or stops.
>
> Measure maximum observed end-to-end command/reply latency, feedback age, tick lateness, missed deadlines, queue overflows and bus error counters. Report the workload, duration, joint count, adapter/USB topology, kernel and clock source.
>
> Exercise both arms and the eventual six-drive leg load in simulation, then qualify the physical topology as it becomes available. Include CPU, disk, camera and USB contention. Vcan results do not qualify wire load or USB timing.
>
> Promotion to 300 Hz or use of PREEMPT_RT requires a recorded comparison against the same acceptance budget. If 250 Hz fails, report that failure and fix the measured cause before powered acceptance.

**(d) Why:** `marengo/hardware/docs/research/can-stack-for-marengo.md` calls bus utilization arithmetic rather than measurement; the firmware behavior document records replies late enough that an assumed collection window needs qualification.

---

**26. SHOULD-FIX — Specify Linux setup and adapter identity precisely.** *(Questions 1, 2, 4)*

**(a) Where:** `roark/PLAN.md`, §3, Linux bullet.

**(b) Current:**

> “Linux: bitrate and sample point set explicitly, error frames on, no automatic bus-off restart while moving, short TX queue.”

**(c) Replace with:**

> For the selected classic-CAN drives, configure 1,000,000 bit/s, sample point 0.75, bus error reporting and TX queue length 10 as the research’s initial settings. Disable automatic bus-off restart during motion. Verify the effective settings at startup rather than trusting setup success.
>
> Bind logical buses to stable physical adapters. USB enumeration order must not decide whether an adapter is the right-arm or left-arm bus. Verify expected drive identities after mapping.
>
> Record the effective scheduling policy, affinity, memory-lock result and privileges. A failed realtime setup is visible and fails timing acceptance.
>
> Retain MCP2515-specific pacing only while a selected bus actually uses the HAT. USB echo timing must be qualified separately; it is not automatically wire-completion timing.

**(d) Why:** These exact initial settings and the candleLight TX-echo qualification appear in `marengo/hardware/docs/research/can-stack-for-marengo.md`.

---

**27. SHOULD-FIX — Define how the simulator is controlled and avoid making MuJoCo a safety-test prerequisite.** *(Questions 1, 2, 6)*

**(a) Where:** `roark/PLAN.md`, §4.

**(b) Current:**

> “…one sim-drive process … driven by MuJoCo, with fault injection built in…”

> “There are no spare actuators, so every stop rule is proven here first.”

**(c) Replace with:**

> The first sim-drive backend is a deterministic protocol/state model on vcan. It supports the messages needed by scan, preflight, enable, reference, cyclic control and stop; independent golden vectors check the shared codec.
>
> A scenario selects the drive inventory, initial register state, seed and fault schedule. Fault events are keyed to simulation time or an observed protocol operation, and include silence, delay, dropped replies, faults, wrong mode, reboot with volatile-setting loss and failed writes.
>
> The scenario runner starts the simulator and unchanged runtime, submits API operations, collects MCAP/candump evidence and checks state transitions and transmitted frames. Each run reports the seed and scenario.
>
> Add MuJoCo coupling when a dynamics, trajectory or replay test needs physical motion. Declare whether each test uses deterministic stepping or wall-clock pacing.
>
> Model protocol faults in sim-drive; exercise transport error handling separately. Vcan does not reproduce physical arbitration, USB buffering or electrical bus-off behavior.
>
> Simulation acceptance proves the implemented response to an injected condition. Physical timeout, caps, adapter timing and E-stop behavior still require supported hardware qualification.

**(d) Why:** `marengo/crates/davout/tests/physical_firmware/mod.rs` already supplies a protocol/state model; the CAN research explicitly limits what vcan can prove.

---

**28. SHOULD-FIX — Make measurement outputs usable and correct stand torque calculations.** *(Questions 1, 2, 4)*

**(a) Where:** `roark/PLAN.md`, §7.

**(b) Current:**

> “…command and reply at about 1 kHz…”

> “Output: one file per actuator type, ready for MJCF `frictionloss`, `damping`, `armature`.”

**(c) Replace with:**

> Qualify the one-drive stand rate separately from the 250 Hz robot loop. Report achieved command, reply and Iq sampling rates, timestamp uncertainty and losses. Parameter-read traffic for `iqf` consumes additional bus budget; do not claim one Iq sample per MIT reply.
>
> Implement T1–T6 as named, bounded test plans with their source procedure, fixture configuration, applicable mode/cap qualification, stop rules and result checks.
>
> For T2, use force in newtons and the perpendicular moment arm. With the documented plumb scale and tilted lever, torque is `force × r × cos(θ)`, including the lever assembly’s own gravity torque. A scale reading in mass units must be converted and its units recorded.
>
> T3 measures compliance of the drive plus fixture; report it as such rather than assigning all measured deflection to gearbox backlash or stiffness.
>
> Each actuator-type result records model, unit identity, firmware, supply, fixture, temperature, raw session, fitting method, units, uncertainty and measured spread. A single tested unit does not establish population spread.
>
> Completion: a recorded run can be refitted without rerunning hardware, and generated MJCF values retain their measurement provenance.

**(d) Why:** `marengo/hardware/designs/actuator-test-stand/README.md` specifies the tilted-scale correction and fixture compliance; `marengo/hardware/docs/sim-to-real-loop.md` specifies the test outputs.

---

**29. SHOULD-FIX — State the minimum build, CI and deployment contract.** *(Questions 1, 2, 6)*

**(a) Where:** `roark/PLAN.md`, after “Names and crate layout.”

**(b) Current:** No new-stack toolchain, CI or Pi deployment contract.

**(c) Add:**

> Pin a Rust toolchain and dependency lockfile after verifying the chosen SocketCAN 4.0 stack builds for the Pi target. The reference pins Rust 1.88.0, but that is evidence about the legacy build, not proof of compatibility with the new dependency set.
>
> CI uses the same pinned toolchain and runs formatting, linting, locked builds, schema/codec contracts and Linux vcan failure scenarios. A missing vcan environment must fail the required integration job rather than silently skip it. CI never connects to hardware.
>
> Deploy an identified binary, skeleton and approved model as one reviewable release. Stop and disable the current runtime before activation; run offline validation before starting the replacement. Startup remains disabled.
>
> Record the installed release identity and retain a known-good rollback. Rollback restores software artifacts but does not auto-enable motion or restore reference validity.
>
> Service setup includes verified realtime privileges, watchdog behavior and ordered independent disable. Completion includes crash/restart, invalid-config and rollback demonstrations on the Pi with drives disabled or simulated.

**(d) Why:** `marengo/rust-toolchain.toml`, `Cargo.toml`, `.github/workflows/ci.yml` and `scripts/systemd/marengo-pi.service` establish the legacy baseline but cannot specify the fresh stack by inheritance.

---

**30. SHOULD-FIX — Correct protocol-table ownership and document the unresolved manual contradictions.** *(Questions 2–4)*

**(a) Where:** `roark/PLAN.md`, §§2–3.

**(b) Current:**

> “…per-model scales from the skeleton…”

> “RS00 MIT scales. Code uses ±50 rad/s / ±17 N·m, every RS00 manual says ±33 / ±14.”

> “Damper 0x702A (RS03 also 0x2026)”

**(c) Replace with:**

> The skeleton selects the actuator model and qualified firmware/protocol profile. The codec owns one versioned protocol table per profile; do not duplicate wire scales in every joint record.
>
> The local RS00 CAN 2.0 manual revision 260713 specifies type-1/type-2 ranges of ±33 rad/s and ±14 N·m; legacy code uses ±50 and ±17. Treat the new RS00 profile as unqualified until a supported loose-drive capture resolves the installed firmware’s behavior. This gate applies to both arms.
>
> Do not copy a nearby parameter table’s numeric ranges over the operation-control frame table. The local RS02 manual’s parameter list gives 14 N·m and 33 rad/s where its operation-control table gives 17 N·m and 44 rad/s. The RS04 parameter list similarly differs from its ±15 rad/s operation-control velocity scale.
>
> Read damper through the qualified private-protocol parameter 0x702A and require zero. The RS03 upper-computer table also lists 0x2026; do not treat these addresses as interchangeable without qualification. Never write damper=1.
>
> Manual disagreements are explicit codec qualification issues, not permission to select the larger range.

**(d) Why:** `marengo/crates/robstride/src/motor_type.rs`, `params.rs` and the four local manuals expose these distinctions; the snapshot contains only one RS00 PDF revision, so “every RS00 manual” was not independently verified here.

---

**31. SHOULD-FIX — Replace stale hardware-status claims with qualified snapshot facts.** *(Questions 3, 4)*

**(a) Where:** `roark/PLAN.md`, “When the hardware is ready.”

**(b) Current:**

> “Upper arm (printed) | Printing”

> “Forearm and RS00 | Designed, not printed”

> “6808 bearings arrive Mon 2026-11-09…”

> “…waiting for Joey's review and print…”

**(c) Replace the affected state cells with:**

> **Upper arm:** STATUS reports printing, but `hardware/designs/left-arm/upper-arm/README.md` says its described rev B is not released for printing or load. Identify the actual printed revision and its release evidence before powered assembly.
>
> **Forearm:** draft 4 is designed but explicitly not released or reviewed. Its yaw-stop tab is described as ±170°, while STATUS lists provisional lower-arm-yaw bounds of ±1.05 rad. Do not populate final hard bounds from either without reconciliation.
>
> **Bearings:** BUY_ORDER records an ordered pair arriving “Monday”; receipt is unverified. The forearm design also requires its own support pair, so confirm actual allocation and condition before assembly.
>
> **Stand:** all four sizes are documented as designed and sliced. Physical printing, assembly and release remain unverified.
>
> **Adapter:** approximately $60 is the research estimate, not a verified current quote. Confirm the purchased board, firmware, isolation and gs_usb operation before acceptance.

**(d) Why:** These qualifications come from `marengo/hardware/STATUS.md`, `hardware/designs/left-arm/upper-arm/README.md`, `forearm/README.md`, `BUY_ORDER.md`, `hardware/designs/actuator-test-stand/README.md` and the CAN research.

---

**32. SHOULD-FIX — Specify replayable log content, clocks and retention without inventing an existing schema.** *(Questions 1, 2)*

**(a) Where:** `roark/PLAN.md`, “Observability.”

**(b) Current:**

> “Every frame in and out goes to MCAP … with kernel timestamps…”

> “…health metrics … go to Prometheus/Grafana.”

**(c) Add:**

> Define the MCAP schema before the first recorded acceptance run. Raw CAN records include bus, direction, identifier/flags, length, bytes, timestamp source, receive/kernel timestamp where available, host monotonic time and local command generation.
>
> Preserve separate meanings for command intent, socket write acceptance, local echo and drive feedback. Do not label all TX timestamps as wire time.
>
> Record decoded joint state with validity and age, requested and admitted commands, applied limits, preflight readbacks, state transitions, stop reasons, operator/API actions, injected faults and dropped-record counters.
>
> Each session groups the submitted plan, code/skeleton/model identities, power/fixture/payload description, MCAP, independent candump files, selected camera evidence and result table. Document the actual directory layout and retention/offload policy when implemented.
>
> Safety deadlines use the Pi’s monotonic clock. Record its relation to wall-clock time and any adapter clock. Synchronize the Jetson when introduced and record measured synchronization uncertainty; a wall-clock adjustment cannot change a safety deadline.
>
> Completion: replay opens a real session, reproduces its command sequence in sim and reports per-joint position, velocity and torque error together with any missing evidence.

**(d) Why:** `marengo/hardware/docs/research/can-stack-for-marengo.md` defines candidate raw fields and approximate USB TX echoes; `marengo/hardware/docs/sim-to-real-loop.md` §§4–5 requires replay and synchronized evidence.

---

**33. SHOULD-FIX — Reconcile “whole robot” with incremental implementation and naming.** *(Questions 1, 3, 6)*

**(a) Where:** `roark/TENETS.md`, §0; `roark/PLAN.md`, introduction.

**(b) Current:**

> “Never for the current iteration of one limb.”

> “If the answer is ‘only for this limb’ or ‘only for now’, it's the wrong change.”

> “Nothing in the code, the docs or the folder layout is named after the current step.”

> “The software is written once…”

**(c) Replace with:**

> Shared runtime code addresses joints and buses through the skeleton and contains no branch selected solely by the current milestone or the robot’s display name.
>
> Implement only capabilities required by the next accepted test. The complete inventory does not require implementing future gait, perception, battery or hand behavior now.
>
> Limb names are valid in actual hardware data, calibration records and fixture-specific procedures. They are not names for shared control architecture.
>
> Changing the selected joint set must not require a new controller implementation. Hardware-dependent behavior is selected by explicit data or capability requirements, not by milestone number.

**(d) Why:** `roark/PLAN.md` itself contains necessary left-arm-specific procedures and deferred power-board work; literal “only for this limb” and “written once” conflict with that incremental scope.

---

**34. SHOULD-FIX — Prevent single-source and safety-tenet wording from deleting necessary evidence or independent protection.** *(Questions 1, 3, 6)*

**(a) Where:** `roark/TENETS.md`, §§2–3.

**(b) Current:**

> “One fact lives in one place. If a joint name, limit or ID appears in two files, one of them is wrong.”

> “A rule that duplicates another layer, or protects nothing nameable, goes.”

**(c) Replace with:**

> Each operational hardware fact has one editable authority. Generated models, API schemas, logs and tests may repeat it as derived or historical data, with provenance. They cannot independently override it.
>
> Every safety mechanism identifies the protected hardware, failure covered, detection condition, action and acceptance evidence. Independent layers may deliberately overlap to cover different failures of the host, bus, drive or power path. Remove redundant policy machinery only after showing that the failure remains covered.

**(d) Why:** `marengo/hardware/electrical/pdb/REV_B_PARTS.md` deliberately requires both drive timeout and hardware heartbeat; generated models and recorded sessions necessarily repeat joint identifiers.

---

**35. SHOULD-FIX — Preserve decided future requirements and keep automation within existing physical authority.** *(Questions 1, 3, 6)*

**(a) Where:** `roark/TENETS.md`, §§4 and 6.

**(b) Current:**

> “Code exists only if a test we're actually running needs it.”

> “Speculative code, ADRs and docs are deleted…”

> “Every tuned number comes from a logged test…”

> “Anything we'd do more than twice becomes a script or loop an agent can run and judge on its own.”

**(c) Replace with:**

> Production code is added for the next concrete acceptance test or a failure that test must handle. Do not create empty future subsystems or placeholder APIs.
>
> Keep agreed future requirements in PLAN, clearly marked as deferred. Remove abandoned proposals and obsolete current instructions; preserve dated measurements and decisions that explain an operational constraint.
>
> Label numeric values as vendor limits, measured values, owner decisions, derived quantities or provisional estimates. Tuned values cite the logged measurement used to choose them. Provisional values do not qualify powered operation.
>
> Repeated procedures should become reproducible tools with explicit inputs, outcomes and stop rules. Automation does not authorize new physical actions: first motion, new stages, power cycling, firmware changes and limit changes retain their stated attended or owner-controlled gates.
>
> Each test names the real failure, safety rule, protocol contract or measured quantity it protects. Completion requires the relevant failure case, not a coverage target.

**(d) Why:** `roark/PLAN.md` contains decided later requirements; `marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md` explicitly restricts unattended actions.

---

**36. SHOULD-FIX — Replace package-count targets and open-ended stack research with concrete boundaries.** *(Questions 1, 6)*

**(a) Where:** `roark/PLAN.md`, layout introduction and observability conclusion; `roark/TENETS.md`, §§7–8.

**(b) Current:**

> “Proposed layout, about 10 crates in place of 15 crates and 6 bins.”

> “Final names are the software project's call…”

> “An agent … should know within minutes what every layer does…”

> “Detailed, concise, timely logs at every step…”

> “A wider observability stack choice … is still to be researched.”

**(c) Replace with:**

> The layout table assigns responsibilities; it is not a target package count or an instruction to create every package now. Create a package only when an implemented boundary or executable needs it.
>
> Keep the initial names unless an implemented boundary makes another name clearer. Robot branding belongs in display data, not identifiers required to build, run or deploy the software.
>
> Maintain one architecture map showing actual processes, ownership, data direction, realtime boundaries and failure behavior. Each implemented package documents its purpose, prohibited responsibilities, callers and dependencies. The entry guide gives the build, vcan run, result location and safe-stop entry points.
>
> MCAP, independent candump and Prometheus/Grafana are the baseline recording and metrics choices. Add other observability products only for a demonstrated missing capability. No separate stack-selection study blocks the first acceptance run.
>
> Observability is complete when the named command, state, fault and timing evidence can be recovered from a run, including a failed run.

**(d) Why:** `marengo/Cargo.toml` verifies the legacy 15-library/6-bin count, but the proposed table lists eight libraries and three Rust binaries; the CAN research already selects the baseline observability tools.

---

**37. SHOULD-FIX — Add a compact factual qualification table to stop copied numbers becoming guarantees.** *(Question 4)*

**(a) Where:** `roark/PLAN.md`, new “Evidence and qualification status” subsection.

**(b) Current:** No consolidated distinction between established values, design choices and unverified claims.

**(c) Add:**

> | Claim | Evidence status |
> |---|---|
> | Five limbs, five CAN buses and 23 body joints | Supported design inventory: two five-joint arms, two six-joint legs and one waist joint. Hands and planned neck servos are outside that 23-joint inventory. |
> | Left arm `can1`, IDs 11–15; right arm `can0`, IDs 1–5 | Supported by hardware STATUS; left elbow/forearm IDs remain marked proposals there. Waist/leg full-body mappings are not settled by the legacy templates. |
> | Left shoulder RS03 ×2 and upper-arm yaw RS02 | Supported hardware assignment. Physical readiness still requires the harness/release gates. |
> | 250 Hz; optional 300 Hz; receive window near 60% | Research-selected starting policy, not a measured Pi qualification result. |
> | SocketCAN 4.0, blocking threads, rtrb and thread-priority | Selected architecture. The research names `netlink` and `dump` features; “candump-log” is a capability description, not an established feature name. |
> | 120 Ω at each bus end, approximately 60 Ω measured unpowered | Supported bench wiring requirement, subject to checking the actual adapter and drive termination. |
> | 1000 timeout units = 50 ms; zero disables it | Supported by the local vendor manuals. Enforcement on installed firmware still needs qualification. |
> | Damper zero | Supported by the parameter tables; damper=1 disables anti-backdrive protection in the manuals that explain it. It is not a holding brake. |
> | Three missed replies | New host policy from the CAN research, not a vendor guarantee or wall-clock bound by itself. |
> | Soft-limit insets of 2–3° supervised and ≥5° unattended | Source procedure requirements; actual limits require released/measured geometry. |
> | Pause at 60 °C, stop at 75 °C | Project limits from the calibration procedure, not the vendor protection thresholds. Resume threshold is ≤45 °C under that procedure. |
> | Elbow 17 N·m; hard stops 0°/124°; approximately 40 N hand side-load restriction | Supported structural/procedural limits, not a continuous powered operating specification or proof that the RS02 bearing is protected. |
> | Stand near 1 kHz, speed cap 1 rad/s except T1, cap margin +10%, RS03→RS02→RS00→RS04 | Source test-plan targets. Achieved sampling, mode enforcement and permissible caps require qualification. |
> | Heartbeat 50–100 ms; clamp 30 W over 60 s and 3 kJ per burst | Supported rev B5 requirements. The average clamp limit remains provisional until the heat test. |
> | BRIO, approved camera plan, rebuilding torso, routing rev B5 board | Supported snapshot status; not evidence of completed assembly or release. |
> | Monday bearing delivery | An order expectation; receipt was not verified. |
> | Six-reviewer panel | Supported by the reconciliation document; its findings are a source of leads, not replacement qualification evidence. |
>
> This packet does not verify current adapter stock or price, actual printed/assembled revisions, installed left-drive firmware, completed structural tests, the Pi’s timing under the intended load, or the effectiveness of each drive limit in MIT mode.

**(d) Why:** The distinctions are supported by `marengo/hardware/STATUS.md`, `hardware/docs/kinematics.md`, the CAN research, `CALIBRATION_LOOP.md`, `sim-to-real-loop.md`, `REV_B_PARTS.md`, the harness, `BUY_ORDER.md` and `review/sw-foundation-20261108/RECONCILIATION.md`.

---

**38. NICE-TO-HAVE — Make the entry files state what a fresh agent may conclude.** *(Questions 1, 3)*

**(a) Where:** `roark/CLAUDE.md`, after its reading list; `roark/README.md`, final paragraph.

**(b) Current:**

> “Work on branches and open PRs. No tests written just for coverage.”

README has no acceptance-status qualification.

**(c) Add to CLAUDE:**

> Work on branches and open PRs. Start with the next unmet PLAN acceptance gate. Report implemented capabilities separately from simulated acceptance and physical qualification.
>
> Do not infer hardware readiness from a milestone name, a responding drive or a provisional skeleton value. Unresolved physical facts block only the affected powered capability; continue independent software and simulation work.
>
> Marengo is reference evidence, not a runtime dependency or an architectural template. Its historical software names may remain in provenance records.

**Add to README:**

> PLAN is a build and acceptance contract, not a claim that its runtime, safety layers or hardware integrations already exist. The skeleton is planned; hardware STATUS and its detailed sources identify what is physically available and released.

**(d) Why:** `roark/README.md` currently describes the target stack in the present tense, while `roark/PLAN.md` is explicitly a rebuild brief.

---

**39. MUST-FIX — Record the decisions that require the owner or physical hardware evidence.** *(Question 7)*

**(a) Where:** `roark/PLAN.md`, new “Owner decisions and physical gates.”

**(b) Current:** No consolidated decision list.

**(c) Add:**

> The following questions remain open. Recommended defaults permit software and simulation work to continue; they do not fabricate physical qualification.
>
> | Question | Recommended default |
> |---|---|
> | Which waist and leg bus/ID assignments are approved? | Preserve joint inventory with unresolved assignments. Do not adopt the conflicting humanoid-template arm/bus plan. |
> | Which exact upper-arm and forearm revisions are released for powered assembly, and which limits belong to them? | Keep installed-arm enable unavailable until hardware identifies the released revisions and bounds. |
> | Does the left bench branch share the right arm’s effective hardware E-stop? | Treat simultaneous powered two-arm testing as unavailable until both branches’ interruption is confirmed. |
> | Is the 22–24 V part of the calibration window intentional despite the manuals’ 24 V operating minimum? | Do not qualify operation below the vendor minimum without explicit evidence. |
> | May an autonomous calibration apply bounded host-offset/model corrections, or only propose them? | Propose changes; hardware commits hardware facts; runtime never rewrites or hot-applies the skeleton. |
> | Does milestone 1 require the complete C0–C8 circuit, or a named subset? | Track every stage separately and reserve “camera calibration complete” for the full required circuit. Do not count C1/C2 as the whole loop. |
> | May a fault remain confined to one supported limb? | Stop all energized motion until narrower containment is demonstrated for that setup. |
> | What physical reference pose and fixture establish zero for each installed joint? | Require an attended reference; do not restore motion permission from a previous process or power cycle. |
> | What constitutes a cooled/reset brake-energy burst? | Keep the 3 kJ budget consumed until a thermally justified reset condition is approved. Do not reset it merely because a trajectory ended. |
> | Who owns the milestone-2 deletion in Marengo? | Marengo’s owning project executes a separate reviewed deletion after Roark supplies the passing migration evidence and preservation list. |
>
> Required bench evidence is separate from owner preference: drive identity/firmware, direction, reference repeatability, RS00 scales, mode-specific cap enforcement, adapter timing, structural release and actual hardware receipt must be measured or inspected. Until then, the affected powered operation remains unavailable.

**(d) Why:** These unresolved points are exposed by `marengo/hardware/STATUS.md`, the harness’s open E-stop question, `CALIBRATION_LOOP.md`, `forearm/README.md`, `REV_B_PARTS.md`, the legacy configuration and the conflicting ownership statements in the four Roark documents.