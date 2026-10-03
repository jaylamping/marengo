# Phase B — WP-L: telemetry honesty (IMU, host metrics, candump, safety publication)

Branch `audit/wp-l`, based on main `223db4a0`. Rule applied throughout: unknown never renders as a healthy zero. A stale or absent sensor reports stale or absent, a missing host reading is unknown, and kernel error frames are visible.

"Red" means the new test, or a scratch probe that uses only the baseline API, failed on main before the fix. Scratch probes ran in a detached worktree under `/tmp` (since removed) or by swapping in the baseline file and then restoring it. Where the baseline had no seam to test against, the evidence is by inspection and says so.

## Commits

| Commit | Scope |
|---|---|
| `23a80ac4` feat(proto) | Append-only fields: `JointState.sample_age_ms`=10, `ImuSample.sample_seq`=16, `HostMetrics.simulated`=32, `ChappeHealth.gateway_probe_latency_ms`=5 (optional), `IpcQueueHealth.expired_total`=13 / `rejected_total`=14, `MemoryMetrics.meminfo_known`=8, `ThermalMetrics.cpu_known`=5, `NetworkInterfaceMetrics.rates_valid`=10, `PiPlatformMetrics.throttle_known`=9. `buf lint` is clean. Each new number is the highest in its message, with no reuse or renumbering |
| `68d26013` fix(marengo-imu) | `poll` returns fresh samples only, plus `sample_seq`. Zero-norm reports are dropped. Errno-based disconnect. Cancellable init. Inbound SHTP sequence gaps are logged. Known skip lengths for 0x03/04/06/08/09 |
| `772e3c55` fix(host-metrics) | Known-flags. Bounded subprocesses (2 s). Exact-mount disks. Throttle bits 0/2/3. Rate validity. Stub marked `simulated`. Linux-only state is cfg-gated (the macOS dead-code warnings are gone) |
| `fa69df2c` fix(chappe) | `rejected` and `expired` counters are split out of `dropped` |
| `7236d14c` fix(davout) | Fault dedupe keeps each distinct cause's message. `ReferenceOutcome::Current` never carries NaN. `JointFeedback.sample_age` is published through Berthier |
| `62b979d4` test(proto) | Wire fixtures for the new fields |
| `da60d8ab` fix(imu) | marengo-pi publisher and imu-probe: shared 7-bit hex address parser, fresh-only counting, error-run restart, rate clamp |
| `fcc8aa7f` fix(marengo-pi) | Measured `gateway_probe_latency_ms` replaces the fabricated `gateway_rtt_ms` |
| `0c23e03b` fix(candump) | Kernel error and RTR frames are parsed frames. The extended flag comes from wire width. Enrichment is direction-aware. `Summary.enriched`. Gateway JSON gains `rtr` and `enriched` |
| `6b8c263a` fix(marengo-pi) | Tick errors are kept until the next successful publish. `software_estop_latched` = Davout latch. The tracing subscriber is installed before Davout construction |
| `16099bbd` chore(consul) | Proto checksum |
| `240baaee` fix(consul) | Static cap tables removed (unknown is shown as "limit unknown"). E-STOP / FAULT LATCHED header states. Fault severity is shown. Host card shows unknowns and the `simulated` source. Candump table shows RTR |
| `7c707a5e`, `5eeda4dd` docs | Codemaps (marengo-imu, host-metrics, crates index), candump module docs, docs/safety.md |

## Verdicts

| Lead | Verdict | Evidence red → green | Fix |
|---|---|---|---|
| L-marengo-imu-01 (CS17) | **CONFIRMED** | `driver::tests::poll_reports_each_sample_once_then_silence`. Red: a scratch `red_second_poll_with_no_new_packet_is_none` against baseline `driver.rs` failed, because the second `poll` with no new packet returned the cached sample | `poll` reports a sample only when one arrived since the last poll (`pending_rotation`), `sample_seq` counts fresh samples, and a reset clears the cache. The publisher sends only fresh samples with `sample_seq`. imu-probe counts distinct reports. 10 consecutive poll errors end the session (restart with backoff) instead of only logging a warning |
| L-marengo-imu-02 | **CONFIRMED** | `bus::errno_tests::unplugged_sensor_is_an_error_not_idle`. Baseline matched `"No such device"` text and returned `NoPacket` (i2c_linux.rs:11). This is Linux-only code and cannot be exercised on macOS, so the evidence is by inspection | Errors are classified by numeric errno (`i2cdev` `Errno`/`raw_os_error`). EREMOTEIO and EAGAIN mean no packet. ENODEV and everything else is a bus error, so the session restarts |
| L-marengo-imu-03 (CS18) | **NEEDS-DECISION** | No raw capture exists. `shtp.rs` still pins 12 bytes | See below |
| L-marengo-imu-04 | **CONFIRMED** | `zero_norm_rotation_is_dropped_not_published`. Baseline `normalize` returned (0,0,0,0) unchanged and it was published (types.rs:14) | `Quaternion::normalize_checked` returns `None` for zero norm, and the driver drops the report |
| L-marengo-imu-05 | **CONFIRMED**, partly fixed | `game_rotation_before_rotation_vector_no_longer_hides_it`, `linear_accel_and_gravity_skip_by_stride`. Baseline stopped the batch at 0x08 | The common SH-2 sensor IDs 0x03/0x04/0x06 (10 bytes) and 0x08/0x09 (12 bytes, same CS18 caveat) are now skipped by stride. A truly unknown ID still ends the batch, because its length cannot be known |
| L-marengo-imu-06 | **ALREADY-FIXED** | `wait_rotation_vector` was deleted in B10 (`dc804fd9`) | — |
| L-marengo-imu-07 | **CONFIRMED** (diagnostic only) | Baseline ignored `buffer[3]` | Per-channel inbound sequence tracking, with gaps and duplicates logged at debug. The driver has no retransmit path, so gaps are not acted on. The continuation bit stays masked, because the I2C read already returns the whole packet |
| L-marengo-imu-08 | **CONFIRMED** | `initialize_while_aborts_when_cancelled` | `Bno085::initialize_while(keep_going)` checks between attempts and inside the 5 s product-ID wait. The publisher passes `!shutdown` |
| L-marengo-imu-10 | **CONFIRMED** | By inspection: `1_000_000 / report_hz` with no upper bound | `MARENGO_IMU_REPORT_HZ` is clamped to 1..=1000 |
| L-imu-probe-01 | **CONFIRMED** | Baseline used `imu.poll()?` (main.rs:136) | Transient errors print a warning. 10 in a row abort |
| L-imu-probe-02 | **CONFIRMED** as an ambiguity, resolved by convention | `address_tests::*`. The Pi's live `/etc/marengo/env` has `MARENGO_IMU_ADDRESS=4b`, and the MCP default is `"4b"` (bare hex) | `marengo_imu::parse_i2c_address` is shared by both bins. It parses hex with or without `0x` (the I2C convention, so existing configs keep working), is limited to 7 bits, and refuses garbage. marengo-pi now disables the IMU with an error log instead of silently falling back to 0x4B |
| L-imu-probe-03 | **CONFIRMED** | Baseline used a fixed 10 ms sleep | Poll at half the report interval (minimum 1 ms). `--report-interval-us 0` is refused |
| L-marengo-host-metrics-01 | **CONFIRMED**, partly addressed | The collector cannot run on macOS | New pure seams with tests: `collect_mounts` (runs on macOS), `parse_throttled` and `parse_service_status` (Linux-only `linux_collector_tests`, run in Pi/Linux CI). The rest of `mod linux` remains I/O glue |
| L-marengo-host-metrics-02 | **CONFIRMED** | Consul `host-unknown-observation.test.ts`. Baseline card mapping: `tempC = cpuCelsius ?? 0`, RAM always shown, `throttled` false when vcgencmd failed. Collector by inspection: missing thermal → 0 °C, meminfo → 0, systemctl failure → Inactive, vcgencmd failure → `throttled_now=false` | `meminfo_known`, `cpu_known` (unreadable zones are skipped, not shown as 0 °C), `throttle_known`. A failed or unrecognised service query is omitted, not reported as Inactive. Consul shows "—" or a muted "throttle ?" |
| L-marengo-host-metrics-03 | **CONFIRMED** | `unmounted_mount_point_is_unknown_not_the_parent_filesystem`. Baseline `collect_disks` admitted `entry.path == "/"` as the covering mount (diagnostics.rs:150-154) | The production sampler uses `collect_mounts`: the mount point itself must be in mountinfo, otherwise the identity and capacity are unknown |
| L-marengo-host-metrics-04 | **CONFIRMED** | `throttled_bits_cover_under_voltage_and_soft_temp` (Linux) | `throttled_now = events & 0b1101` (under-voltage, throttled, soft temperature limit). The raw mask stays in `throttle_events` |
| L-marengo-host-metrics-06 | **CONFIRMED** | By inspection: `Command::output()` had no timeout | `run_bounded` kills the helper after 2 s and reports unknown. The marengo-pi gateway probe was already bounded at 200 ms |
| L-marengo-host-metrics-07 | **CONFIRMED** | By inspection: `saturating_sub` turned a counter reset into a 0 rate, and the baseline map never shrank | `rates_valid=false` on the first sample, a counter decrease or a stall. Vanished interfaces are dropped from `prev.network` |
| L-marengo-host-metrics-08 | **CONFIRMED** | By inspection: `unwrap_or(1.0).max(0.001)` | Gaps longer than 5 s invalidate the rates. The first sample is invalid |
| L-marengo-host-metrics-09 | **CONFIRMED** | Baseline stub had hostname "dev-host" and a fabricated budget, with no marker | `simulated: true` on the stub (false from Linux). Consul shows a "simulated" badge and footer |
| L-marengo-pi-10 | **CONFIRMED** | `disconnected_queue_observations_survive_host_health_wire_conversion` now carries a latency. Baseline published `gateway_rtt_ms: 0.0`, hard-coded | `gateway_rtt_ms` is removed. `gateway_probe_latency_ms` (`optional double`) is the measured TCP-connect time of the probe that sets `gateway_reachable`, and it is absent when the probe fails |
| L-chappe-05 | **CONFIRMED** | `ipc_backpressure::absent_peer_has_bounded_classes_and_honest_counters`. Red on the baseline outbox: `dropped` 994 included the unknown-topic and oversize admissions. Also `unknown_topic_and_oversize_are_rejected_not_dropped` | Unknown topic, oversize payload and closed-outbox admissions now count as `rejected`, not `dropped`. Wire field `rejected_total` |
| L-chappe-08 | **CONFIRMED** | `expired_state_counts_expired_inside_dropped` | `expired` counter (a subset of `dropped`), wire field `expired_total` |
| L-davout-12 | **CONFIRMED** | `faults::record_tests::distinct_drive_state_causes_on_one_address_stay_distinguishable`. Red: scratch `red_second_cause_survives_dedupe` on baseline `faults.rs` lost "missing Off echo" | A deduped record appends each distinct new cause (up to 1024 chars). Repeating the same cause does not grow the message |
| L-davout-22 | **CONFIRMED** (latent) | By inspection: `unwrap_or(f32::NAN)`. A public-API reproduction is not reachable, because the binding check already refuses non-finite evidence | `Current` only for a finite evidence position, otherwise `Failed { "usable reference without finite evidence position" }` |
| L-davout-23 | **CONFIRMED**, data fixed, semantics **NEEDS-DECISION** | By inspection: TTL 5 s vs grant `comm_watchdog_ms` | `JointFeedback.sample_age` → `JointState.sample_age_ms` (demo publisher sends 0, which is true for synthesized data). docs/safety.md states the difference. See below |
| L-armee-proto-07 | **CONFIRMED** | `enable-feedback.test.ts` "safety publication honesty". Red on baseline `enable-feedback.ts` (2 failed). `machine-state-safety.test.ts` (no baseline seam) | `software_estop_latched = snapshot.is_latched()` (proto comment updated). Consul header shows `E-STOP` / `FAULT LATCHED` ahead of the mode, `formatSafetyFaults` labels E-STOP and warning severity, and post-enable feedback reports an asserted hardware E-stop |
| L-marengo-pi-05 | **CONFIRMED** | `safety_publication_tests::one_tick_error_is_retained_until_a_publication_succeeds`. The baseline loop assigned `active_fault = None` on every healthy tick (main.rs:1793). The loop itself has no unit seam | `UnpublishedTickFault` keeps the first tick error (plus a count of later ones) until a publish succeeds |
| L-marengo-pi-07 | **CONFIRMED** | By inspection: `init_subscriber` ran after `ControlLoop::from_repo_with_physical_reference` | The subscriber is installed right after config-dir resolution, before config loads, the SocketCAN open and Davout construction |
| L-consul-01 | **CONFIRMED** | `telemetry-gauge-limits.test.ts`. Baseline fell back to `GAIN_LIMITS.rs03` (60 Nm, 50 rad/s) and ±π. `DISPLAY_STATIC_JOINT_LIMITS` was also wrong (kp 50 vs 500/5000, roll/pitch velocity 2.0 vs 2.5) and had no caller | `resolveGaugeLimits`: live snapshot → master config → null ("limit unknown", no bar). `GAIN_LIMITS` (gauge), `DISPLAY_STATIC_JOINT_LIMITS`, `staticLimitsForJoint` and the uncalled `resolveJointLimits` are deleted. The type is renamed `JointCapLimits` |
| L-marengo-candump-01 | **CONFIRMED** | `kernel_error_frame_parses_without_enrichment`, `remote_request_shapes_parse_with_empty_data`, `malformed_lines_count_but_do_not_parse` (now 4). Red: scratch `red_error_frame_counted` on main gave `parsed_frames` 0 for `20000004#` | `CanId` accepts the `CAN_ERR_FLAG` range (`is_error`). RTR is parsed in both shapes (`ID#R[n]`, `[n] remote request`) with `Frame.rtr`. Both count in `parsed_frames`/`top_ids` and are never enriched |
| L-marengo-candump-02 | **CONFIRMED** | `extended_flag_comes_from_wire_width`, `can_id_tests::extended_flag_comes_from_wire_width_not_value`. Red: scratch `red_extended_low_id` (`000001FE` was read as standard) | `CanId { value, extended }`. The extended flag is the ID field width (more than 3 hex digits). JSON deserialization infers it the same way |
| L-marengo-candump-03 | **CONFIRMED** | `scan::enrichment_tests::*` (10 tests). Red: scratch `red_host_type24_attribution`: host type-24 `1800FD02` gave `device_id` 253 | `motor_device_id` follows the `wire::classify_frame` direction rule with `DEFAULT_HOST_ID`. Type-0 replies decode the responder from the marker; type-17 replies use `decode_read_parameter_reply`; host frames use the low byte. Truncated reply-shaped frames resolve to nothing |
| L-marengo-candump-05 | **CONFIRMED**, visibility fixed | `summary_enriched_reflects_joint_resolution` | `Summary.enriched` (true only when a frame resolved a joint) is exposed in CLI text and gateway JSON. A catalog that fails to load still warns once at gateway start. Reloading the catalog at runtime is out of scope (it is loaded once by design) |

## Prune rows

WP-L has no batch of its own. B9, B10 and B12 are already merged. Rows that touch this package:

| Row | Outcome | Why |
|---|---|---|
| P-marengo-host-metrics-07 (`gateway_rtt_ms`) | **Deleted** (field removed from `ChappeHealthInput`; it was never on the wire) | Replaced by the measured, optional `gateway_probe_latency_ms` |
| P-marengo-host-metrics-06 (`gateway_reachable` + probe) | **Kept** | The probe now carries a real measurement and feeds the honest health surface. Deleting it needs a proto deprecation wave (contracts P26) |
| P-marengo-imu-09 (test pinning 12-byte rotation) | **Kept** | Gated on the CS18 raw capture (see NEEDS-DECISION) |
| P-marengo-imu-03 (accel/gyro parsers) | **Kept** (KEEP row) | — |
| Consul `DISPLAY_STATIC_JOINT_LIMITS` / `staticLimitsForJoint` / `resolveJointLimits` (contracts C26) | **Deleted** | Wrong values and no live caller |
| Consul `pid-slider-panel.tsx` `GAIN_LIMITS` | **Kept** | It holds slider ranges for the unwired tuning route that D-11 keeps, not displayed limits |

## NEEDS-DECISION

### CS18 / L-marengo-imu-03: rotation-vector report stride (12 vs 14 bytes)
- **A (recommended): capture first.** Run `scripts/pi-bno085-shtp-init.py` on the bench IMU and record one raw channel-3 batch with 0x05 followed by another report. Set `REPORT_ROTATION_VECTOR` (and 0x08/0x09) to the observed stride, and replace `split_batch_two_reports` with the captured fixture. Cost: one bench session. Every published quaternion stays trustworthy.
- **B: switch to 14 now (Adafruit/CS18).** If the firmware really sends 12 bytes (commit `6439835`'s observation), every batch misaligns after the first rotation report and the bench IMU breaks.
- **C: keep 12.** If the firmware sends 14, a report after a rotation vector in the same batch is dropped or misparsed. Today only 0x05 is enabled, so the visible effect is limited to batched rotation reports.

### L-davout-23: one "online" notion
`sample_age_ms` now lets consumers tell "heard within 5 s" from "fresh". Options:
- **A (recommended): Consul marks a joint "stale" when `sample_age_ms` exceeds `comm_watchdog_ms`,** keeping Online/Offline on the 5 s TTL. This is a display change only and needs the watchdog value in the config snapshot.
- **B: shrink `FREE_DRIVE_FEEDBACK_TTL` to `comm_watchdog_ms`.** The 2.5 s Consul status poll would then flap joints Offline between polls unless type-24 streams are on.
- **C: publish grant liveness explicitly** (a new proto field from the reference authority). This is the most precise option and adds a second facet to `JointState`.

## Behaviour that changes on the bench

- **IMU:** `sensors/imu/torso` is published only on fresh reports, so a silent or unplugged sensor stops publishing. ENODEV and a run of 10 poll errors restart the session. An invalid `MARENGO_IMU_ADDRESS` disables the IMU with an error log. The Pi value `4b` stays valid.
- **Host metrics:** older Consul builds will see `rates_valid`/`*_known` as false only from a new Pi build. The new Consul treats an old Pi build's `totalBytes > 0` and a non-zero temperature as known, but shows throttle state as "throttle ?" until the Pi is redeployed.
- **Chappe `dropped_total`:** no longer rises with Consul command traffic (now `rejected_total`).
- **Candump summaries** (`pi_candump_summary`, gateway, log-cli): `parsed_frames` and `top_ids` now include kernel error frames (`2000xxxx`) and RTR frames. Low extended IDs print as 8 digits.
- **Consul header** shows `FAULT LATCHED` whenever Davout holds a fault (previously `DISABLED`).

## Cross-package edits

- `crates/berthier/src/loop.rs`: one field, `sample_age_ms`, in `publish_robot_state`.
- `crates/davout/src/lib.rs`: the `JointFeedback.sample_age` field and its value. `reference_commit.rs`: L-davout-22.
- `crates/marengo-store/src/store.rs`: `enriched: false` in the two empty-summary literals.
- `bins/marengo-gateway/src/logs.rs` (JSON `rtr`/`enriched`) and `webtransport.rs` (demo `sample_age_ms`).
- `crates/armee-proto/src/lib.rs` and `crates/marengo-homing/src/commissioning.rs`: test literals for the new proto fields.
- Consul: `data/actuator-joints.ts`, `state/actuatorStore.ts`, `data/host-metrics.ts`, `lib/log-api.ts`, `site-header-status-badges.tsx`, `logs/candump-frame-table.tsx`.

## Gate

Single run on `audit/wp-l` (macOS arm64, Rust 1.88):

```
cargo fmt --all -- --check                                              fmt: ok
cargo clippy --workspace --all-targets --exclude marengo-host-metrics \
  --exclude marengo-pi -- -D warnings                                   Finished (0 warnings)
cargo clippy -p marengo-pi --all-targets \
  --target aarch64-unknown-linux-gnu -- -D warnings                     Finished (0 warnings)
cargo test --workspace                                                  exit 0; 121 result lines, passed=1207 failed=0
cd consul && npm test -- --run                                         Test Files 78 passed (78); Tests 385 passed (385)
cd consul && npm run build                                             ✓ built in 2.08s
```

Extra checks:
- `cargo clippy -p marengo-host-metrics --all-targets -- -D warnings` on macOS is clean. The dead-code warnings that excluded this crate are gone.
- `cargo clippy -p marengo-pi -p imu-probe --features marengo-pi/linux-i2c,marengo-pi/socketcan,imu-probe/linux-i2c --target aarch64-unknown-linux-gnu -- -D warnings` is clean, which covers the feature-gated `imu.rs` and `i2c_linux.rs`.
- `buf lint` (in `proto/`) is clean. `buf breaking` could not resolve the git input from a worktree (`.git` is a file). Every new field number is new and highest in its message, and no field was removed or renumbered, so the change is append-only by construction.
- `scripts/proto-checksum.sh`: ok.

After the gate, `mod host_metrics` in marengo-pi was cfg-gated to Linux to match its only caller. This clears the four pre-existing macOS dead-code warnings. Re-checked with `cargo test -p marengo-pi` (99 passed, no warnings), aarch64 clippy with and without the IMU/SocketCAN features, and `cargo fmt --check`.

### Merge with main 02571ec9 (B5/B11, B7/B13, WP-R, WP-MQ)

- Conflicts: `crates/armee-proto/src/lib.rs` took main's side (P-armee-proto-01 deleted the prost round-trip tests; WP-L only touched them to fill the new struct fields). `consul/src/gen/.checksum` was regenerated with `npm run gen:proto`; `scripts/proto-checksum.sh` reports ok.
- Proto: main added deprecations only, no new field numbers, so WP-L's appended fields do not collide. `buf lint` is clean, and so is `buf breaking --against '/Users/joseph/code/marengo/.git#branch=main,subdir=proto'`.
- Semantic check: `chappe::ipc_outbox` now takes topic lookup from `chappe::topics` (WP-MQ) and keeps the WP-L dropped/expired/rejected split. In marengo-pi, the subscriber and Bus are still created before config load, `UnpublishedTickFault` still feeds `publish_safety` (now `TOPIC_SAFETY`), and `mod host_metrics` is still Linux-only.
- Gate: `cargo fmt --check` ok; both clippy commands `-D warnings` ok; `cargo test --workspace` 1235 passed, 0 failed (124 suites); Consul `npm test -- --run` 384/384 (78 files), `npm run build` ok; MCP `npm test` 207/207.
