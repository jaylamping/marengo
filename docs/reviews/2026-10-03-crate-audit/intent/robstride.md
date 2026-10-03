# Intent card: `robstride`

## 1. Header

| Field | Value |
|---|---|
| Crate | `robstride`, the Robstride RS00–RS04 CAN driver |
| Path | `crates/robstride` |
| Kind | lib. Features: `socketcan` (Linux SocketCAN backend) and `vcan = ["socketcan"]` (`Cargo.toml:13-16`) |
| Baseline | `a2b55b3` (worktree `audit/2026-10-03`) |
| LOC | src 3,772 total, about 2,918 non-test (`bus.rs` 1,529; `lib.rs` 575 of which 481 are the test module; `params.rs` 462; `mit.rs` 237; `receive.rs` 207; `feedback.rs` 203; `identity.rs` 126; `lifecycle.rs` 120; `comm.rs` 119; `motor_type.rs` 81; `command.rs` 72; `state.rs` 41). tests/ 1,851 in 7 files (`receive_contract.rs` 597, `command_validity.rs` 446, `feedback_report.rs` 338, `transport_boundary_vcan.rs` 212, `fault_evidence_regressions.rs` 99, `socketcan_conversion.rs` 87, `transport_regressions.rs` 72). metrics/loc.md:32 |
| Coverage (macOS, default features, so the `socketcan` module is not compiled) | crate 88.2% lines (metrics/coverage-by-crate.md:31). Per file: bus.rs 77.9%, receive.rs 83.1%, motor_type.rs 65.4%, command.rs 92.9%, mit.rs 95.0%, params.rs 98.6%, comm/feedback/identity/lifecycle 100% (metrics/coverage-by-file.md:25,37,53,84,96,116,123-126) |
| Sources consulted | `src/lib.rs:1-38` //! docs; `README.md`; `codemap.md`, `src/codemap.md`; AGENTS.md:41-66,122-123,156-158,199,281; crates/AGENTS.md:14,17,34,42-43,56; crates/codemap.md:16-17,40,51,59; docs/rust-patterns.md:26-29,42-44,96-115,328-335; docs/safety.md:10; hardware/docs/decisions/0002-robstride-protocol.md; ADR 0020, 0021, 0036; RS02 User Manual 260713 and RS03 User Manual 260713 (vendor PDFs from the ADR links, §4.1.2–4.1.9); Seeed `RobStride_Control` SDK `python/robstride_dynamics/{protocol,table,bus}.py`; prior review control.md (CS01, CS03, CS04, CS06, arch gap 1), test-quality-plan.md:59, tooling.md T21, finding-index.md, implementation-ledger.json (CS03, CS04, M02), implementation-roadmap.md:573-588; `git log -- crates/robstride` (37 commits); `cargo tree -i robstride`; repo-wide greps of consumers; `cargo check -p robstride --all-targets` (clean); metrics/*.md |

## 2. Intent

Robstride is the **motor-space transport driver**. It turns approved MIT commands, lifecycle requests (Enable, Disable, Set Zero, type-24 reporting) and firmware register writes into 29-bit extended Robstride CAN frames. It turns received frames back into ordered, addressed, lossless evidence (status pose, status flags and drive mode, raw type-21 detailed fault and warning words, malformed partials, kernel error frames, type-0 identity replies, type-17 parameter replies, and this host's own command echoes). Sources: `lib.rs:1-23`; codemap.md:4; hardware ADR 0002:43; ADR 0020:16-20; ADR 0021:16-35; ADR 0036:143-145.

It deliberately holds **no control or safety policy**. Torque and position limits, enable decisions, joint sign and gearing, and fault authority all belong to Davout, which reads Robstride's `FeedbackReport` (`lib.rs:24-29`; AGENTS.md:52,66; hardware ADR 0002:69-81). It does own two things: a bounded, fair, non-blocking receive engine (64 frames and 256 read attempts per poll; ADR 0021:27-44), and a last-line **command validity** guard that rejects nonfinite values, negative gains and wrong register kinds (lib.rs:12; CS03 fix).

It is the only crate meant to know the wire format. AGENTS.md:199 and crates/AGENTS.md:56 forbid rebuilding arbitration IDs at call sites.

Conflicting statements of intent:
- **"No safety filtering"** (codemap.md:4) against "typed rejection of nonfinite input, negative gains" (lib.rs:12) and the ADR 0002 2026-09-30 update (hardware ADR 0002:48-56). The validation is input validity, not policy. The two docs disagree only in wording.
- **Caller.** The README (README.md:3) says "Berthier calls it after Davout approves". lib.rs:35 and AGENTS.md:46 say Davout is the sole production path. The code agrees with Davout: only `davout` calls `MotorBus` (see §4).
- **SocketCAN loopback.** The hardware ADR appendix (0002:103-112) says loopback and `CAN_RAW_RECV_OWN_MSGS` are "vcan only; production path unchanged". Since `edb8fb3` the code enables both on every socket (`bus.rs:1229-1234,1258`), and codemap.md:20,39 says so.
- **Receive compatibility API.** The legacy `recv_all*` and `recv_frames*` projections were removed in B6; consumers use bounded `recv_feedback_report*` and `recv_raw_report` (§9).

## 3. Owns / Must not

| Owns | Evidence |
|---|---|
| 29-bit ID pack/unpack, `CommunicationType` {0,1,2,3,4,6,17,18,21,24}, `DEFAULT_HOST_ID=0xFD`, direction-aware inbound device id | `comm.rs:5-82` |
| MIT type-1 encode (BE u16 p/v/kp/kd, torque in ID bits 8–23) and type-2/24 decode (pose, temp ×0.1, flags bits 16–21, mode bits 22–23) | `mit.rs:106-170`; `feedback.rs:10-28` |
| Per-model MIT scales | `motor_type.rs:18-50` |
| Lifecycle frames: Enable, Disable (data all zero), SetZero (`data[0]=1`), type-24 on/off | `lifecycle.rs:5-61` |
| Parameter IDs, kinds, read-only set, type-17/18 encode, type-17 reply decode | `params.rs:6-302` |
| Type-0 UID request and reply decode | `identity.rs:41-71` |
| `CanBus`/`MotorBus` traits, batch admission (validate whole batch and routes before first TX) | `bus.rs:260-614`, `398-442` |
| Receive envelope (Data/Remote/Error, actual DLC, host timestamp) and SocketCAN conversion (FD rejected) | `bus.rs:116-257` |
| Bounded receive engine, completion classes | `receive.rs:7-207` |
| Lossless `FeedbackReport` assembly, including echo/reply/transport routing | `bus.rs:645-982`; `feedback.rs:86-203` |
| Backends: `MemoryBus`, `RuntimeBus`, `SocketCanBus`, `SocketCanRouter` | `bus.rs:984-1174,1214-1526` |

| Must not (source) | Status |
|---|---|
| Decide torque limits, enable, E-stop (lib.rs:26; AGENTS.md:52) | Respected. Enable/Disable are encode-only and every caller is Davout (§4) |
| Apply direction or gear_ratio (lib.rs:27; hardware ADR 0002:73) | Respected. No `direction` or `gear` in src (grep) |
| Run control or gravity (lib.rs:28) | Respected |
| Load YAML or URDF (lib.rs:29) | Respected. Reads `MotorsConfigFile` only as a parameter to `SocketCanRouter::open` (`bus.rs:1391`) |
| Callers must not rebuild arbitration IDs (AGENTS.md:199; crates/AGENTS.md:56; rust-patterns.md:96) | **Partly violated outside the crate.** No production code outside robstride *builds* IDs. Production Davout does *decode* comm types by hand: `davout/src/simulation.rs:90` (`frame.id >> 24`), `reference_transaction.rs:1260`, `reference_journal_event.rs:269-270` (magic `2`, `17`), `feedback_consumer.rs:646` (magic `21`). Tests hand-build many IDs (`davout/tests/command_admission.rs:320,364,409`; `receive_bounds.rs:56`; `torque_output_contract.rs:91`; `davout/src/lib.rs:4124,4229,4320` via `pack_ext_id(..)|(2<<22)`; Berthier tests at `feedback_bootstrap.rs:44-64` and others). test-quality-plan.md:59 *wants* independent literal fixtures, so the test cases are arguably intentional. `pack_ext_id` (raw `u8` comm type) is public (`comm.rs:65`; `lib.rs:59`), which makes the bypass easy |

## 4. Interface

| Group | Key items | Consumers (crate:file) |
|---|---|---|
| ID codec | `pack_ext_id`, `unpack_ext_id`, `CommunicationType`, `DEFAULT_HOST_ID`, `comm::inbound_motor_device_id`; `ExtendedId`, `EXTENDED_ID_MASK`, `comm::pack_typed_ext_id` | davout `src/lib.rs` (tests only for `pack_ext_id`), `active_reporting.rs:427-459`, `reference_transaction.rs`; marengo-candump `scan.rs:459-462`. `ExtendedId`, `EXTENDED_ID_MASK` and `pack_typed_ext_id`: no external use (grep) |
| MIT | `MitCommand` (+`validate`), `MitFeedback`, `encode_mit`, `motor_type::MitRanges` (wire scales) | Davout sends addressed MIT batches; status decode is internal to the receive-report path. Removed: `decode_mit_feedback`, `mit_tx_id`, `mit_rx_id`, and `MitRanges` convenience bounds |
| Lifecycle encoders | `encode_{enable,disable,set_zero_position,active_reporting}` (+`default_`) | davout `simulation_reference.rs` (`encode_default_set_zero_position` only). The rest are used only inside robstride |
| Params | `ParameterId`, `ParameterValue`, `RunMode`, `ParameterKind`, `ParameterReadReply`, `encode_{read,write}_parameter`, `encode_{set_run_mode,speed_ref,position_ref,current_ref}`, `decode_read_parameter_reply` | davout `lib.rs:163,1713,2271-2277`, `reference_transaction.rs:1008,1323` (MechPos, RunMode, LimitSpeed, SpeedTarget). Free encoders: no external use |
| Identity | `DeviceUid`, `DEVICE_ID_REPLY_MARKER`, `decode_device_id_reply`, `encode_*get_device_id`, `DeviceIdReply` | davout `reference.rs`, `reference_physical.rs`, `reference_transaction.rs` (`DeviceUid`); `DEVICE_ID_REPLY_MARKER` in davout tests only |
| Feedback evidence | `FeedbackReport`, `FeedbackObservation`, `FeedbackEvent`, `MalformedFeedback`, `MalformedReason`, `DetailedFaultFeedback`, `DriveMode`, `HostEchoObservation`, `EchoedCommand`, `IdentityObservation`, `ParameterReadObservation`, `TransportObservation`, `MotorState` | davout `feedback_consumer.rs`, `faults.rs`, `reference_physical.rs`, `lib.rs` |
| Receive engine | `ReceiveLimits`, `ReceiveCompletion`, `ReceiveAttempt`, `RawReceiveReport`, `MAX_RX_*` | davout `feedback_consumer.rs`, `reference_transaction.rs`, `simulation.rs`; marengo-pi tests |
| Bus traits | `CanBus` (required `send_frame`, `recv_one_nonblocking`; overridable `receive_source_count`, `begin_receive`, `echoes_transmissions`, `validate_address`, `send_frame_to`, `recv_raw_report`). `MotorBus: CanBus` exposes addressed `_at` command methods plus `recv_feedback_report{,_with_limits}`; legacy/unaddressed projections were removed | davout uses addressed methods and reports (`lib.rs:1712-1713,1819,2181,2271-2297,2352-2380,2454`; `reference_transaction.rs:1002-1061`; `active_reporting.rs`) |
| Bus adapters | `MemoryBus`/`MemoryRxQueue`, `RuntimeBus::{socketcan, socketcan_from_motors}`, `SocketCanBus`, `SocketCanRouter`, `ReceivedCanFrame::{full_data,new_data,new_remote,from_socketcan}`, `TimedCanFrame`, `CanFrame`, `MotorAddress`, `AddressedMitCommand`, `BusError` | marengo-pi `main.rs:53,1087` and motor-repl `main.rs:14,160-167` (construction only); davout plus Berthier/marengo-pi tests (`MemoryBus`, `full_data`) |
| Features | `socketcan` enabled by `marengo-pi/socketcan`, `motor-repl/socketcan` (bins Cargo.toml:15), deploy scripts (`deploy-pi.sh:193`), CI vcan job (`ci.yml:324`). `vcan` is **never enabled** (no caller in Cargo.toml, scripts, compose or CI) |
| `vcan` module | `vcan::DEFAULT_INTERFACE` | robstride `lib.rs` tests only (`lib.rs:492-567`) |

**Depth (codebase-design).** Codec modules hide wire details behind compact encoders and typed data. `MotorBus` is now a narrower addressed seam: Davout uses the `_at` operations and report APIs; the unaddressed command aliases and legacy receive projections have been removed. The underlying `CanBus` seam remains one-source/one-attempt reads plus routed writes, source rotation and echo capability. `SimulationBus` overrides `recv_feedback_report` (`davout/src/simulation.rs:659-680`).

Adapter count for `CanBus`:
- Production: `SocketCanBus`, `SocketCanRouter`, wrapped by the `RuntimeBus` enum (`bus.rs:1053-1172`), and Davout `SimulationBus`.
- Test and bench: `MemoryBus`, plus 4 robstride test buses (`fault_evidence_regressions.rs:64`, `receive_contract.rs:43`, `feedback_report.rs:125`, `command_validity.rs:105`, plus `ScriptBus`/`RoutedMemoryBus` in `lib.rs:122-197`), and 11 more in davout and marengo-pi tests or examples (`davout/tests/{reference_boundary_public,reference_history,physical_firmware/mod}.rs`, `davout/examples/can_tick_bench.rs:86`, `davout/src/active_reporting_pacing_tests.rs:170`, `marengo-pi/src/{shutdown_tests,safety_receive_diagnostic_tests,safety_publication_tests}.rs`).

The seam is real, not hypothetical. `echoes_transmissions` has one production `true` (SocketCAN) and one test `true` (`davout/tests/physical_firmware/mod.rs:445`).

**Protocol coverage against the vendor manuals** (RS02/RS03 manuals 260713 §4.1):

| Type | Vendor meaning | Implemented |
|---|---|---|
| 0 | get device ID / UID | encode + reply decode (`identity.rs`) |
| 1 | MIT operation control | encode (`mit.rs:106`) |
| 2 | status feedback | decode (`mit.rs:149`) |
| 3 | enable | encode + host echo |
| 4 | stop. `Byte[0]=1` clears faults. Version-read variant `00 C4` | encode with zero payload only; **no fault-clear or version-read frame** |
| 6 | set mechanical zero (`Byte[0]=1`) | encode + host echo |
| 7 | set CAN ID | absent |
| 17 / 18 | single param read / write | encode + type-17 reply decode |
| 21 | fault feedback (bytes 0–3 fault, 4–7 warning) | raw 8-byte retention, no bit decode (by design, ADR 0020:46-48) |
| 22 | save parameters | absent (ADR 0036:143-145 forbids its use) |
| 23 | baud rate | absent |
| 24 | active report | encode on/off, decode as status, Off-echo |
| 25 | protocol switch | absent |

The ParameterId table covers 14 of the vendor's dozens of 0x70xx registers (`params.rs:23-41`). The Seeed SDK lists about 29 (`protocol.py` `ParameterType`).

## 5. Invariants owned

| Invariant | Enforcing code | Test(s) that would fail | File coverage |
|---|---|---|---|
| Nonfinite field or negative gain → typed error, **no frame**; whole batch validated before the first TX | `mit.rs:28-38,106-107`; `bus.rs:398-415,417-442` | `command_validity.rs:26,48`; `lib.rs:221` | mit 95.0%, bus 77.9% |
| Addressed batch: command/address device match, no duplicate address, every route preflighted | `bus.rs:417-436`; `SocketCanRouter::validate_address` `bus.rs:1433-1441` | `command_validity.rs:136,331`; `lib.rs:244` | bus 77.9% |
| Register writes: read-only refused, kind must match, run_mode in {0..3}, gains/limits ≥0, floats finite | `params.rs:126-174` | `params.rs:439`; `command_validity.rs:170,211,250,274,348` | params 98.6% |
| Only extended Data with exactly 8 bytes becomes status or detailed fault; short/RTR/invalid → `Malformed` with raw bytes, no pose | `bus.rs:777-833`; `ReceivedCanFrame::payload` `bus.rs:231-246` | `transport_regressions.rs:12`; `receive_contract.rs:77,113,134`; vcan `transport_boundary_vcan.rs:51,71,131` (CI only) | bus 77.9% |
| Kernel Error frames are transport evidence, never vendor-decoded; full error mask kept; FD refused | `bus.rs:208-226,681-687` | `socketcan_conversion.rs:36,51,61,75` (Linux + feature only) | not in macOS coverage |
| Status flags (bits 16–21) and drive mode (22–23) kept separately from pose | `mit.rs:166-168`; `feedback.rs:19-28` | `feedback_report.rs:27`; `fault_evidence_regressions.rs:27` | 95–100% |
| Type-21 all 8 raw bytes kept; nonzero fault or warning visible | `feedback.rs:34-55`; `bus.rs:972-982` | `feedback_report.rs:64`; `fault_evidence_regressions.rs:46` | feedback 100% |
| A fault-only or warning-only report never refreshes or creates pose time | `feedback.rs:157-177` | `feedback_report.rs:151,180`; `command_validity.rs:412` | 100% |
| One raw ordinal across all event lists; the first terminal error is ordered before later peer frames | `receive.rs:149-154`; `bus.rs:678` | `receive_contract.rs:156,191`; `feedback_report.rs:89,198` | receive 83.1% |
| Per-poll bound of 64 frames / 256 attempts; limits can only narrow; noise and interruptions spend quota | `receive.rs:104-105,121-124,135-148` | `transport_regressions.rs:37,59`; `receive_contract.rs:215,231,316` | 83.1% |
| Partial pass or cap never claims Idle/Quiet; clock overflow or expired deadline fails before IO | `receive.rs:87-99,125-133,157-187` | `receive_contract.rs:231,341,360` | 83.1% |
| Router fairness: one read per source per round, rotating start | `bus.rs:1497-1515`; `receive.rs:113,157` | `transport_boundary_vcan.rs:95`; `lib.rs:481` (both ignored, vcan) | not in macOS coverage |
| Host echo = exact Enable / reporting-Off / SetZero bytes from host 0xFD to a configured address; never feedback | `bus.rs:718-749,840-878` | `receive_contract.rs:501,553,584` | 77.9% |
| SocketCAN backends report `echoes_transmissions()==true` only after own-message echo was configured, else open fails | `bus.rs:1229-1234,1258,1373-1376,1518-1522` | **untested in robstride.** No vcan test checks that own frames arrive. Only Davout's fake `FirmwareBus` returns true |
| Type-0/17 replies count only when complete, from a configured address, addressed to this host; never pose | `bus.rs:711-716,880-946`; `identity.rs:58-71`; `params.rs:283-302` | `lib.rs:421`; `identity.rs:120`; `params.rs:423` | 97–100% |
| Ordinary Disable payload never clears firmware faults (ADR 0020:40) | `lifecycle.rs:19-21` | `feedback_report.rs:280` | 100% |
| Positive-budget complete empty poll → benign `RecvTimeout`; zero-budget empty poll → no error | `bus.rs:660-669` | `feedback_report.rs:266`; `receive_contract.rs:360` | 77.9% |
| Atomic single nonblocking write, short write reported, no retry | `bus.rs:1311-1326` | **untested** | not in macOS coverage |
| Duplicate configured address refused at router open | `bus.rs:1394-1407` | **untested in robstride** | not in macOS coverage |

## 6. Inputs/outputs

- **Config:** `MotorType` (`marengo_config`, `mit.rs:3`); `MotorEntry.{can_interface, device_id}` → `MotorAddress` (`bus.rs:103-107`); `MotorsConfigFile.motors[].{joint, can_interface}` for the router (`bus.rs:1391-1420`). No YAML is read directly.
- **CAN TX:** types 0, 1, 3, 4, 6, 17, 18, 24 with host 0xFD (`comm.rs:63`), classic 8-byte extended frames (`bus.rs:360-392`).
- **CAN RX:** types 0, 2, 17, 21, 24 decoded. Types 3, 6 and 24-from-host are host echoes. Types 1, 4, 18 and unknown types are ignored with a `trace!` (unknown types with no trace, `bus.rs:708-710`). Kernel error frames are kept.
- **OS:** SocketCAN via the `socketcan` 3.x crate. Socket is nonblocking, with loopback and recv-own-msgs, and the error filter accepts all classes (`bus.rs:1243-1273`).
- **Env, files, HTTP, proto, Chappe:** none. Tracing only (`bus.rs:24-39,1244,1268,1288,1345`).

## 7. Prior review reconciliation

| Prior id | Current status | Evidence |
|---|---|---|
| CS01 (empty drain `Ok(0)` renews watchdog; evidence `bus.rs:378-381`) | **Superseded in robstride, fixed in Davout.** The engine reports completion and `RecvTimeout` for positive budgets (`bus.rs:660-669`). A zero-budget empty drain still returns no error by design (`feedback.rs:180-183`). Per-motor freshness lives in Davout. Ledger: verified PR214 | `receive.rs:30-43`; `feedback_report.rs:266` |
| CS03 (NaN encodes extreme torque) | **Fixed** at the driver boundary | `mit.rs:28-38,106-107`; `params.rs:157-172`; `command_validity.rs:26,48,348`; ledger verified |
| CS04 (flags dropped, detail truncated, healthy status erases fault) | **Robstride part fixed. Finding is partial overall** (firmware byte-order qualification and recovery still open; ledger) | `mit.rs:166-168`; `feedback.rs:34-55,157-177`; `fault_evidence_regressions.rs:27,46,87`; batch04 commit `6d5cedf` |
| CS04/M06 bounded ingress (ADR 0021) | **Fixed** | `receive.rs`; `bus.rs:1339-1371,1497-1515`; ledger CS04 evidence (batch04, run 36684361659) |
| CS06 (Set Zero verified against stale feedback) | **Partial.** Robstride now supplies the plumbing: exact SetZero echo (`15542aa`), type-0 UID, type-17 MechPos replies (`88f2b9e`). Correlation lives in Davout; bench qualification pending | `bus.rs:718-726,880-946`; implementation-roadmap.md:573-588 |
| control.md arch gap 1 / M02 (drive `CanTimeout` and torque limit never written or read back) | **Open.** `ParameterId::CanTimeout`/`LimitTorque` exist (`params.rs:29,34`) but have zero uses outside robstride; ledger M02 "open" | grep `ParameterId::` outside robstride: only MechPos, RunMode, LimitSpeed |
| test-quality-plan.md:59 (independent protocol fixtures, full-width faults, NaN; vCAN in Linux CI) | **Done** | `command_validity.rs:375` (independent RS03 wire fixture); `feedback_report.rs:27,64`; `ci.yml:282-324` runs `--include-ignored` with `socketcan` |
| T21 (daily-audit CAN-bypass false positives on test-only robstride imports) | **Unverifiable from this crate.** Tooling finding (`scripts/daily-audit/audit.py`), not re-checked here | tooling.md:244-250 |

## 8. Drift

1. hardware ADR 0002:103-112 says loopback and `RECV_OWN_MSGS` are vcan-only, names `configure_vcan_loopback` (removed in `edb8fb3`) and says "production path unchanged". Code: `bus.rs:1229-1258` enables both on every socket. The ADR was last touched in `d4c869b` (2026-09-29).
2. hardware ADR 0002:1 titles the file "ADR 0006 (hardware)" while its filename is 0002.
3. hardware ADR 0002:31-38 "Rated limits (Seeed table)". The values are the **MIT wire scales** from the Seeed SDK `table.py` (RS00 17 Nm / 50 rad/s), not rated limits. Seeed's own product table gives RS-00 as 14 Nm / 315 rpm. The RS03 manual 260713 §4.1.2 gives a ±20 rad/s MIT velocity range, while ADR, code and SDK say 50 (see lead L1).
4. hardware ADR 0002:96-97 previously named the removed `decode_mit_feedback` and `recv_all` projections. The production decoder is internal to `recv_feedback_report`; ADR 0002 now documents this report-owned path.
5. README.md:3 claims "Berthier calls it after Davout approves" and "on CAN1". The real caller is Davout (lib.rs:35); `config/motors.yaml` uses `can0` for all 5 motors.
6. The obsolete `robstride::send(cmd)?` examples in `AGENTS.md` and `docs/rust-patterns.md` are corrected; the Rust pattern now routes joint-space batches through `Supervisor::send_mit_batch`.
7. Davout's `src/lib.rs` data-flow comment now names its actual addressed batch operation (`mit_control_all_at`).
8. crates/codemap.md:59 mentions a "`SyntheticBus` in Marengo for integration tests". No such type exists (grep).
9. crates/codemap.md:40 says "no other crate calls `MotorBus`". That is true, but marengo-pi and motor-repl construct `RuntimeBus` and marengo-candump imports `robstride::comm`. lib.rs:31-36 lists only Davout and tests as callers.
10. `crates/robstride/codemap.md` correctly describes `CanBus` as requiring one-attempt reads with no default bulk-drain projections; B6 removed the legacy projections.
11. The MCP Disable wording is corrected (`motion.ts`): ordinary type-4 Disable stops torque but does not clear a latched drive fault. Implementing type-4 Byte[0]=1 remains a recovery-policy decision (L-robstride-02).
12. lib.rs:1 says "(MIT Mode 0)". The crate also drives firmware Speed mode (`bus.rs:531-543`), and Davout uses it (`davout/src/lib.rs:2271-2297`).

## 9. Prune candidates

Usage below is per repo-wide grep over crates/, bins/, tools/, consul/src, scripts/. metrics/pub-usage.md is a narrower heuristic (crates/*/src only) and flags `p_min/p_max/t_min/t_max/v_min` (pub-usage.md:30-34) and `has_warning` as test-only (pub-usage.md:56).

| Candidate | Evidence class | Confidence | Deleting touches |
|---|---|---|---|
| `MitRanges::{p_min,p_max,v_min,v_max,t_min,t_max}` (`motor_type.rs:52-80`) | zero references (Berthier's `v_max` hits are other types) | high | motor_type.rs only. Coverage 65.4% comes mostly from these (metrics coverage-by-file.md:25) |
| Cargo feature `vcan = ["socketcan"]` (`Cargo.toml:16`) | feature never enabled (CI and compose use `--features socketcan`, `compose.yaml:66`; `ci.yml:324`) | high | Cargo.toml only. `motor-repl`'s own `vcan` feature is separate |
| `send_motion` + `JointMotion` (`bus.rs:1184-1212`), "legacy position path" with kp=kd=0 (misleading: no position stiffness) | zero references outside robstride; one internal re-export (`lib.rs:53`) | high | bus.rs, lib.rs re-export |
| `send_mit` free fn (`bus.rs:1195-1198`) | zero production references (davout `lib.rs:61` is a doc comment); duplicates `mit_control_all` | high | bus.rs, lib.rs:53, rust-patterns.md:108, davout lib.rs:61 doc |
| Unaddressed `MotorBus` methods `mit_control_all`, `enable_drive`, `disable_drive`, `set_zero_position`, `read_parameter`, `write_parameter`, `set_run_mode`, `speed_control` (`bus.rs:398-415,444-534`), plus `send_encoded_frame` (`bus.rs:360-370`) | duplicate of the `_at` variants; zero production callers outside robstride. `SocketCanRouter::send_frame` fails for more than one interface without an address (`bus.rs:1443-1460`) | med (robstride's own tests use them: `lib.rs:221`, `command_validity.rs`) | bus.rs; robstride tests (`lib.rs:200-243`, `command_validity.rs`, `feedback_report.rs`) |
| Legacy receive projections `recv_frames`, `recv_frames_from`, `recv_frames_from_nonblocking`, `recv_timed_frames_from`, `recv_timed_frames_from_nonblocking` (`bus.rs:304-357`); `RawReceiveReport::into_result` | zero references outside robstride; superseded by `recv_raw_report` (ADR 0021:67-70 frames them as compatibility, but no compat consumer remains) | med | bus.rs, receive.rs:64-77; tests `transport_regressions.rs:37-72`, `receive_contract.rs:374` (these are **baseline-regression tests that pin a public API**, codemap.md:67) |
| `recv_all`, `recv_all_addressed`, `finish_feedback_projection`, `FeedbackObservation::update_state` (`bus.rs:555-591,616-643`; `feedback.rs:154-178`) | zero references outside robstride; superseded by `recv_feedback_report` (codemap.md:23 "Davout safety must consume FeedbackReport") | med (they pin the CS04 baseline regressions `fault_evidence_regressions.rs`, `receive_contract.rs`, `lib.rs:268-420`) | bus.rs, feedback.rs; about 15 robstride tests |
| `mit_tx_id`, `mit_rx_id` (`mit.rs:57-68`) | zero references outside robstride; doc says "remains for callers/tests" | high | mit.rs tests, lib.rs:75 |
| `decode_mit_feedback` (`mit.rs:127-145`) | no production consumer; production uses `decode_status_payload` | low (pinned by `transport_regressions.rs:12` baseline test and the ADR text) | mit.rs, transport_regressions.rs |
| `encode_position_ref`, `encode_current_ref` (`params.rs:229-248`) | no consumer; README.md:9 lists them as supported. Only `command_validity.rs` tests use them | low (bench-diagnostic intent stated in hardware ADR 0002:46) | params.rs, lib.rs:77, tests |
| `MitFeedback.fault` (`mit.rs:49-50,166`) | duplicate of `status_flags` (same value, widened to u16) | low (feeds `MotorState.fault`, which Davout reads) | mit.rs, feedback.rs:165 |
| `FaultReport.device_id` (`bus.rs:73-77,978-981`) and the unreachable `else { continue }` (`bus.rs:814-819`) | private field never read; the branch is dead because type and length are already checked at `bus.rs:762-787` | low | bus.rs |
| `vcan` module (`lib.rs:87-93`) | only consumer is robstride's own tests (`lib.rs:492-567`) | low | lib.rs |
| Public `pack_ext_id` (raw `u8` type) re-export (`lib.rs:59`) | enables the hand-built-ID pattern; used only in davout tests | low (narrowing breaks davout test fixtures, which test-quality-plan.md:59 wants independent) | davout `src/lib.rs` tests, `tests/physical_firmware/mod.rs` |
| `RuntimeBus` non-socketcan stubs that always error (`bus.rs:1066-1088,1100-1101,1176-1182`, 5 `let _ =` suppressions in metrics/suppressions.md:724-728) | scaffold for non-Linux builds | low (needed to compile bins on macOS) | keep. Simplification only |

Not prunable: `ParameterId::{CanTimeout, LimitTorque, EPScanTime, ZeroSta, AddOffset}`, which have no writers. CanTimeout and LimitTorque are the M02 gap (lead L4). ZeroSta and AddOffset are decode vocabulary per ADR 0036:143-145. The disable/stop path is likewise not prunable.

## 10. Phase-B leads

1. **L1 — RS03 MIT velocity scale disagrees with the cited primary manual.** The RS03 User Manual 260713 §4.1.2 (PDF p.41; the hardware ADR 0002:14 links it) maps type-1 `v_des` and type-2 velocity to **±20 rad/s**. The code uses 50 (`motor_type.rs:35-40`), matching the Seeed SDK `table.py` and ADR 0002:36. If the installed firmware matches the manual, RS03 velocity commands and feedback (right shoulder pitch and roll, `config/motors.yaml:10,24`) are wrong by 2.5×. That affects the kd·(v_des−v) term and Davout's velocity checks. The RS02 manual's ±44 does match the code (RS02 manual §4.1.2). Needs a bench check: jog at a known speed and compare decoded velocity against position differentiation.
2. **L2 — No fault-clear capability, while tooling claims there is one.** Vendor type-4 `Byte[0]=1` clears faults (RS02/RS03 manual §4.1.4). `lifecycle.rs:19-21` always sends zeros, and ADR 0020:52 says "no fault-reset capability". Yet `pi_motor_disable`/`pi_motor_recover` present Disable as the "primary fault clear" (`tools/marengo-pi-mcp/src/tools/motion.ts:633`). Operators may believe faults were cleared when they were not. This is a gap, not a prune.
3. **L3 — Echo attribution is host-id-based, not socket-based.** `ingest_host_echo` (`bus.rs:840-878`) accepts any exact Enable, SetZero or reporting-Off frame with host 0xFD. With loopback on, frames from **other local sockets** (motor-repl, `cansend`, a second marengo-pi) look identical (comment at `bus.rs:729-730`). [INFERENCE] Linux reports `MSG_CONFIRM` for the socket's own TX and `MSG_DONTROUTE` for local origin. These flags are not read because `read_frame` hides them. Davout gates SetZero and Enable ordering on these echoes (`davout/src/reference_transaction.rs:954-1219`; `lib.rs:1474`). The risk is mitigated only by the MCP's sole-CAN-owner discipline.
4. **L4 — Drive-local timeout and torque limit never configured (M02).** `ParameterId::CanTimeout` and `LimitTorque` have no writer or readback outside robstride. A hung or killed host leaves the drives executing the last MIT command (vendor: `CAN_TIMEOUT=0` disables the drive's own protection, RS02 manual §3.3.6). Gap.
5. **L5 — The receive cap plus TX echo halves headroom.** Every TX is now echoed into the same 64-frame and 256-attempt budget (`receive.rs:7-8`; codemap.md:39). A `WorkLimit` completion latches a Transport fault in Davout (`davout/src/feedback_consumer.rs:589-608`). At 200 Hz (`config/control.yaml:3`), the humanoid profile has 23 motors on 5 buses (`config/motors_humanoid.yaml`): 23 echoes + 23 statuses ≈ 46 frames per poll before any type-24, identity or late frames. One delayed poll could exceed 64 and trip the robot. [INFERENCE] There is no throughput measurement (ADR 0021:32-33 says the limits are not measured). batch36 already observed controller RX overflow on can0 (ledger CS04).
6. **L6 — A single foreign frame can fail a whole poll.** An unexpected FD frame or RTR with DLC > 8 returns `Err` from `from_socketcan` (`bus.rs:197-226`) → terminal error → `Failed` → Davout transport latch. This fails closed, but any foreign node or misconfigured adapter on the bus becomes a stop trigger. Confirm the policy is intended.
7. **L7 — An echo-only poll is reported as `RecvTimeout`.** In `feedback_from_raw` (`bus.rs:660-669`) the "empty" test ignores `host_echoes`. With a positive budget, a poll that read only this host's echoes gets `terminal_error = RecvTimeout`. Davout waits on echoes with positive budgets during reference phases (ADR 0036:47,63-68). Check whether Davout treats `RecvTimeout` as benign in every branch.
8. **L8 — Run-mode switch while enabled.** `MotorBus` lets callers write `run_mode` at any time. Davout enables and then writes `RunMode::Mit` (`davout/src/lib.rs:1712-1713`), and switches to Speed while enabled (`lib.rs:2271`). Vendor precaution 2: "Do not switch the control mode when the joint is running; send stop before switching" (RS02/RS03 manual, Precautions). Robstride encodes it without any guard. Cross-crate lead.
9. **L9 — Untested transport guarantees.** Not tested: own-message echo actually arriving on SocketCAN (`bus.rs:1229-1258,1374`); atomic short-write reporting (`bus.rs:1322-1326`); duplicate-address refusal at `SocketCanRouter::open` (`bus.rs:1394-1407`). The whole `socketcan` module is outside the macOS coverage numbers (bus.rs 77.9% excludes it). Gap.
10. **L10 — Open error misclassified.** A `CanSocket::open` failure maps to `BusError::Send` (`bus.rs:1245-1247`), not `Driver`, so callers matching on Send-vs-Driver misreport a missing interface as a TX failure.
11. **L11 — Silent drop of unknown comm types.** `bus.rs:708-710` `continue`s with no `trace_skipped_frame`. Types 5, 7, 22, 23 and 25 (another host doing set-ID or baud change) vanish from diagnostics, while every other skip is traced.
12. **L12 — Blocking sleep in the receive path.** `receive.rs:189-195` calls `std::thread::sleep` (at least 200 µs) between idle passes. All current callers are synchronous (Davout or the marengo-pi loop; the gateway does not depend on robstride per `cargo tree -i`). Any future async caller would block a worker.
13. **L13 — Ambiguous interface-less matching.** In `address_for_frame` (`bus.rs:951-970`), when `interface=None` (MemoryBus, or `recv_all`, which strips the interface at `bus.rs:569-571`) and the same device id exists on two buses, the frame is dropped as "unconfigured". The behaviour is safe, but evidence is silently lost in tests that use MemoryBus with multi-bus configs.
14. **L14 — Type-21 byte order unqualified.** Byte order is kept raw (`feedback.rs:30-55`, ADR 0020:46-48). The Seeed SDK decodes it as little-endian `<LL` (`bus.py` `receive_status_frame`). Davout's fault messages and the Consul display should not invent bit names until a bench capture confirms the order.
