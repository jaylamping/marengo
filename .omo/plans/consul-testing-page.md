# Plan: Consul "Testing" Page with Runtime Gain Overrides
# Scope: Full-stack feature across proto, gateway, berthier/davout, and Consul React frontend
# Author: Prometheus / Sisyphus-Junior
# Date: 2026-06-21

## 1. Goal

Add a Consul "Testing" page that lets an operator:
1. Select any combination of actuators (multi-select, with group presets like "entire limb", "both elbows").
2. Run a **hold test** at a chosen position (radians).
3. Adjust `kp`/`kd`/`ki`/`fc` in real time via sliders and see actuator behavior change **within the next control cycle** without restart.
4. View real-time telemetry: **torque** (`JointState.effort`) and **velocity** (`JointState.velocity`) vs configured limits.
5. Retain the existing `dryRun` safety toggle and safe defaults.

## 2. Key Design Decisions

### 2.1 Backward-Compatible Proto Extension
- Add **optional** `ki` and `fc` to `MitJointCommand`. Existing consumers that ignore unknown fields are safe. Protobuf binary wire format is forward-compatible for added optional fields.
- `ki` and `fc` are **not** sent to the motor firmware (Robstride MIT frame has no fields for them). They are consumed by Berthier to override the integral term and friction model at runtime.
- `kp` and `kd` already exist in `MitJointCommand` and are already sent every tick. We simply allow Consul to send non-zero values that override the config defaults.

### 2.2 Runtime Override Architecture in Berthier
- Berthier currently does **not** subscribe to `robot/command`. It generates its own `MitJointCommand`s internally from `control.yaml`.
- We will add a **Chappe bus subscription** to `robot/command` in `marengo-pi`'s startup (or inside Berthier's `ControlLoop`).
- Incoming `MitCommandBatch` envelopes will be parsed. For each joint in the batch, we store a **runtime gain override** in a new `HashMap<String, GainOverride>` inside `ControlLoop`.
- In `tick()`, when computing gains, Berthier checks the override map first. If an override exists and is **not expired** (TTL ~500ms), it uses the overridden `kp`/`kd`/`ki`/`fc`. Otherwise, it falls back to `control.yaml` values.
- The TTL ensures that if Consul disconnects or stops sending, the robot reverts to safe config defaults within ~500ms (2-3 ticks at 200Hz).
- Gain ramp is **bypassed** for runtime overrides — the override values are applied immediately on the next tick. This is acceptable because the operator is intentionally tuning and expects immediate response.

### 2.3 Gateway: No Structural Changes
- The gateway already accepts `MitCommandBatch` on `POST /command/mit` and publishes it to Chappe topic `robot/command`.
- Since we only add optional fields to the proto, the gateway's decode→re-encode→publish path works unchanged.
- **Verification**: confirm that `armee_proto::MitCommandBatch` decodes successfully even with new fields (prost ignores unknown fields by default).

### 2.4 Consul Frontend
- Create `src/pages/testing.tsx` and add `/testing` route.
- Reuse existing patterns:
  - `useTestingStore` for test state (already exists, unused).
  - `useRobotModel()` for joint list and limits.
  - `Slider` from `components/ui/slider.tsx` for gain/setpoint controls.
  - `JointTrackingChartCard` / `JointTrackingAreaChart` for telemetry (reuse chart components, but add a new chart type for torque/velocity vs limits).
- Extend `testingStore.ts`:
  - Add `groupPresets` (e.g., "Right Arm", "Left Leg", "All Elbows").
  - Add `holdPositionRad`.
  - Change `startTest()` to send `MitCommandBatch` in a loop (or on every slider change) when in hold mode.
  - Debounce slider changes at ~50ms to avoid flooding the network.
- Add a new telemetry chart component `TestingTelemetryChart` that shows:
  - Torque (`effort`) with `torque_limit_nm` reference lines.
  - Velocity (`velocity`) with `velocity_max_rad_s` reference lines.
  - Uses the same Recharts pattern as `JointTrackingAreaChart`.

### 2.5 Safety
- `dryRun` flag in `testingStore` prevents any HTTP POST when true.
- Override TTL in Berthier prevents stale gains from persisting forever.
- Davout's existing safety filters (kp/kd clipping to motor-type max) remain in effect.
- Berthier's existing torque/velocity limit checks in `tick()` remain in effect.

## 3. Files to Modify

### Wave 0: Proto Schema (Blocks all other waves)
| File | Action | Anchor |
|------|--------|--------|
| `proto/marengo/v1/marengo.proto` | Add `ki` and `fc` to `MitJointCommand` | After line 117 (`double kd = 3;`) |
| `proto/marengo/v1/marengo.proto` | Add `ki` and `fc` to `MitJointCommand` | After line 121 (`double torque_ff = 6;`) |

### Wave 1A: Consul Frontend (Parallel with 1B after proto regen)
| File | Action | Anchor |
|------|--------|--------|
| `consul/src/state/testingStore.ts` | Extend store: add `holdPositionRad`, `groupPresets`, debounced slider dispatch, `sendGainUpdate()` helper | Existing `startTest()` function |
| `consul/src/pages/testing.tsx` | Create new Testing page component | New file |
| `consul/src/routes/config.tsx` | Add `/testing` lazy route | Existing route list |
| `consul/src/data/sidebar-nav.ts` | Add "Testing" nav item with real URL | Existing placeholder entries |
| `consul/src/components/testing/testing-telemetry-chart.tsx` | New chart component for torque/velocity vs limits | New file |
| `consul/src/components/testing/joint-multi-select.tsx` | Multi-select actuator picker with group presets | New file |
| `consul/src/components/testing/gain-slider-panel.tsx` | Panel with 4 sliders (kp/kd/ki/fc) + value readouts | New file |

### Wave 1B: Gateway (Parallel with 1A; verify only)
| File | Action | Anchor |
|------|--------|--------|
| `bins/marengo-gateway/src/http.rs` | Verify `MitCommandBatch::decode` tolerates new fields | `command_mit` handler (line 231) |
| `crates/armee-proto/build.rs` | Ensure proto regeneration includes new fields | Existing build script |

### Wave 2: Berthier Runtime Overrides (Depends on Wave 0)
| File | Action | Anchor |
|------|--------|--------|
| `crates/berthier/src/loop.rs` | Add `runtime_gains: HashMap<String, GainOverride>` field to `ControlLoop` | `ControlLoop` struct definition |
| `crates/berthier/src/loop.rs` | Add `GainOverride` struct with `kp, kd, ki, fc, expires_at: Instant` | New type near `GainRamp` |
| `crates/berthier/src/loop.rs` | Add `apply_runtime_gains()` method: checks map, falls back to config | Near `target_gains_for_mode()` (line 602) |
| `crates/berthier/src/loop.rs` | Modify `tick()` to call `apply_runtime_gains()` before computing control law | Lines 708–1006 (mode branches) |
| `bins/marengo-pi/src/main.rs` | Add Chappe bus subscription to `robot/command` topic | `run_control_loop()` or `drain_chappe_commands()` |
| `bins/marengo-pi/src/main.rs` | Add handler: parse `MitCommandBatch` from envelope, call `loop_ctrl.set_runtime_gains(batch)` | Near existing chappe command handlers |
| `crates/berthier/src/loop.rs` | Add `set_runtime_gains(batch: MitCommandBatch)` public method | `ControlLoop` impl block |

### Wave 3: Davout (No changes required, but verify)
| File | Action | Anchor |
|------|--------|--------|
| `crates/davout/src/lib.rs` | Verify `MitJointCommand` struct already has `kp`, `kd` — no new fields needed because `ki`/`fc` are consumed only by Berthier | `send_mit_batch` filter logic |

## 4. Detailed Implementation Notes

### 4.1 Proto Extension
```protobuf
message MitJointCommand {
  string name = 1;
  double kp = 2;
  double kd = 3;
  double position = 4;
  double velocity = 5;
  double torque_ff = 6;
  double ki = 7;   // NEW: integral gain override (Berthier-only)
  double fc = 8;   // NEW: Coulomb friction gain override (Berthier-only)
}
```
- Defaults: `0.0` for both. A value of `0.0` from Consul means "use config default" (since `0.0` is not a valid tuning gain for most joints). Alternatively, use a sentinel or wrap in `google.protobuf.DoubleValue` — but for simplicity, we treat `0.0` as "no override" because real gains are always positive. If a joint truly needs `ki=0`, the operator can set it explicitly in `control.yaml`.
- **Better approach**: Use `optional double ki = 7;` and `optional double fc = 8;` (proto3 `optional` generates `has_ki()` / `has_fc()` accessors in prost). This is cleaner and unambiguous.

### 4.2 Berthier Override Logic
```rust
// In ControlLoop struct
runtime_gains: HashMap<String, GainOverride>,

struct GainOverride {
    kp: f64,
    kd: f64,
    ki: f64,
    fc: f64,
    expires_at: Instant,
}

// In tick(), before reading config gains:
let (kp, kd, ki, fc) = if let Some(ov) = self.runtime_gains.get(name) {
    if Instant::now() < ov.expires_at {
        (ov.kp, ov.kd, ov.ki, ov.fc)
    } else {
        self.runtime_gains.remove(name);
        self.config_gains_for(name) // fallback
    }
} else {
    self.config_gains_for(name)
};
```
- TTL: 500ms. Each incoming `MitCommandBatch` refreshes the TTL for the joints it contains.
- If a joint is **not** in the batch, its override is **not** refreshed and will expire naturally.

### 4.3 Consul `testingStore` Changes
- Add `sendGainUpdate()` that constructs a `MitCommandBatch` with `ControlMode.POSITION` (or `IMPEDANCE` if preferred) and the current slider values, then calls `postMitCommandBatch()`.
- Debounce: use a 50ms trailing debounce so rapid slider movements coalesce into one POST.
- `startTest()` should:
  1. Set `isRunning = true`.
  2. Send the initial batch with current setpoint and gains.
  3. Start an interval (e.g., 100ms) that re-sends the batch to keep the TTL fresh. OR, rely on slider changes to refresh. **Decision**: use a 200ms heartbeat interval while `isRunning` is true, sending the current slider values. This keeps overrides alive even if the operator is not touching sliders.
- `stopTest()` should:
  1. Set `isRunning = false`.
  2. Clear the heartbeat interval.
  3. Send one final batch with `ControlMode.DISABLED` (or just stop sending and let TTL expire). **Decision**: send `ControlMode.DISABLED` batch for explicit safety.

### 4.4 Telemetry Chart
- New component `TestingTelemetryChart` takes:
  - `jointName: string`
  - `data: { time: string; torque: number; velocity: number }[]`
  - `limits: { torque_limit_nm: number; velocity_max_rad_s: number }`
- Renders two `<AreaChart>` rows (or a dual-axis chart) with:
  - `ReferenceLine` for torque limit (red dashed).
  - `ReferenceLine` for velocity limit (blue dashed).
  - `ReferenceArea` for ±80% of limit (yellow warning zone).
- Reuse `ChartContainer`, `ChartTooltip`, `useThrottledValue` from existing chart infrastructure.

### 4.5 Group Presets
- Define in `testingStore.ts`:
```typescript
const GROUP_PRESETS: Record<string, string[]> = {
  'Right Arm': ['right_shoulder_pitch', 'right_shoulder_roll', 'right_elbow', 'right_wrist'],
  'Left Arm': ['left_shoulder_pitch', ...],
  'Both Elbows': ['right_elbow', 'left_elbow'],
  'All': /* all revolute joints from useRobotModel() */
};
```
- UI: `<Select>` dropdown for preset, plus individual checkboxes for fine-tuning.

## 5. Parallel Waves & Dependency Matrix

```
Wave 0: Proto Extension
  └─> Regenerate protobuf bindings (buf generate, cargo build armee-proto)

Wave 1A: Consul Frontend          Wave 1B: Gateway Verification
  (depends on Wave 0)               (depends on Wave 0)
  │                                 │
  │                                 └─> Verify decode tolerance
  │                                 └─> No code changes expected
  │
  └─> Extend testingStore.ts
  └─> Create Testing page + components
  └─> Add route + nav

Wave 2: Berthier Runtime Overrides
  (depends on Wave 0)
  (can run in parallel with Wave 1A/1B, but integration test needs 1A)
  │
  └─> Add GainOverride struct + map
  └─> Add set_runtime_gains() method
  └─> Modify tick() to check overrides
  └─> Add Chappe subscription in marengo-pi

Wave 3: Integration & Verification
  (depends on Wave 1A, 1B, 2)
  │
  └─> End-to-end test: slider move → HTTP POST → Chappe → Berthier → CAN
  └─> Verify TTL expiry reverts to config
  └─> Verify dryRun prevents POST
  └─> Verify torque/velocity charts render
```

## 6. Verification Scenarios

### 6.1 Happy Path
1. Open `/testing` in Consul.
2. Select "Right Shoulder Pitch" from actuator list.
3. Set `holdPositionRad = 0.5`.
4. Click "Start Hold".
5. Observe in network tab: `POST /command/mit` with `ControlMode.POSITION`, `position=0.5`, `kp=12`, `kd=0.75` (defaults from store).
6. Move `kp` slider to `20`.
7. Observe new POST within 50ms with `kp=20`.
8. On Pi: `marengo-pi` logs show `robot/command` envelope received.
9. Joint holds at 0.5 rad with increased stiffness.
10. Telemetry chart shows torque and velocity within limits.

### 6.2 Edge Cases
- **Empty selection**: `startTest()` should be a no-op (or show toast). No POST sent.
- **Zero gains**: If operator sets `kp=0`, the POST is still sent. Berthier applies `kp=0` (gravity comp only). This is valid behavior.
- **Stale override**: Stop sending from Consul. After 500ms, Berthier reverts to `control.yaml` gains. Verify by checking `loop.rs` logs or by observing stiffness change.
- **Multiple joints**: Select 3 joints. Batch contains 3 `MitJointCommand`s. Each gets its own override entry.
- **Single joint**: Same as happy path, but batch has length 1.

### 6.3 No Regression
- **Existing Dashboard chart**: `JointTrackingChartCard` still works, unaffected.
- **Existing `POST /command/mit`**: If an old client sends a batch without `ki`/`fc`, gateway and Berthier handle it (optional fields, fallback to config).
- **Config patches**: Still require restart. Runtime overrides do not persist to disk.
- **Position mode**: Existing position-hold logic (slew, deadband, integral anti-windup) still works; overrides only replace the gain values.

### 6.4 Safety
- **Dry run**: Toggle `dryRun=true`. Move sliders. No network requests. `isRunning` stays false.
- **Limits clamping**: Set `kp=99999`. Davout clips to motor-type max. Verify in `davout/src/lib.rs` filter.
- **Torque limit exceeded**: If torque approaches `torque_limit_nm`, chart reference area turns red. Berthier's existing torque limit logic triggers fault if exceeded.
- **Emergency stop**: E-stop button on page sends `ControlMode.DISABLED` batch immediately.

## 7. Acceptance Criteria & Verification Commands

### 7.1 Proto / Rust
- [ ] `cd proto && buf generate` succeeds.
- [ ] `cd crates/armee-proto && cargo build` succeeds.
- [ ] `cd bins/marengo-gateway && cargo clippy -- -D warnings` passes.
- [ ] `cd crates/berthier && cargo test` passes.
- [ ] `cd crates/davout && cargo test` passes.

### 7.2 Consul / TypeScript
- [ ] `cd consul && npm run typecheck` passes.
- [ ] `cd consul && npm run lint` passes.
- [ ] `cd consul && npm run test` (vitest) passes.
- [ ] `cd consul && npm run build` succeeds.

### 7.3 Manual QA
- [ ] Navigate to `http://localhost:5173/testing` (dev) or hosted Consul URL.
- [ ] Select a joint, start hold, verify actuator moves to setpoint.
- [ ] Adjust `kp` slider, feel/see stiffness change within 100ms.
- [ ] Adjust `ki` slider, observe integral windup behavior (torque drift).
- [ ] Stop test, verify actuator disables or holds with config defaults.
- [ ] Open browser DevTools → Network, confirm debounced `POST /command/mit` requests.
- [ ] Toggle dry run, confirm no network requests.

### 7.4 Mock Server (for CI / offline dev)
- Create a mock Chappe gateway endpoint that echoes `MitCommandBatch` back on a WebSocket stream.
- Use it to verify Consul's POST logic and chart rendering without a physical robot.
- **File**: `consul/src/mocks/mock-chappe-server.ts` (optional, for dev only).

## 8. Open Questions / Risks

1. **Prost `optional` support**: Prost supports proto3 `optional` since v0.9. Verify the `armee-proto` build environment uses a compatible `prost` version. If not, use `double ki = 7;` with `0.0` sentinel.
2. **Chappe bus subscription in `marengo-pi`**: The current `main.rs` only subscribes to `robot/enable` and `robot/homing`. Adding a `robot/command` subscription may require a new async task or extending `drain_chappe_commands()`. Verify that `bus.subscribe("robot/command")` does not conflict with the IPC fanout.
3. **Heartbeat interval in Consul**: Sending a batch every 200ms while holding may be noisy. Alternative: extend TTL to 1000ms and only send on slider change. **Decision**: 200ms heartbeat is safer (faster stale detection) and the payload is small (~100 bytes). Acceptable.
4. **Impedance vs Position mode**: The store currently uses `ControlMode.POSITION`. For pure gain tuning, `IMPEDANCE` might be more appropriate (no position trajectory slew). **Decision**: keep `POSITION` because the user explicitly asked for "hold test at varying degrees" (position hold). The gains override the impedance values used inside the position controller.

## 9. Rollback Plan

- Revert proto changes (remove `ki`/`fc` fields). Protobuf forward compatibility means old binaries ignore unknown fields; removing fields is backward-compatible if no code references them.
- Revert Berthier: remove `runtime_gains` map and `set_runtime_gains()`.
- Revert Consul: delete `testing.tsx` and related components; remove route.
- Gateway requires no rollback (no changes).

## 10. Estimated Effort

| Wave | Subsystem | Estimated Time |
|------|-----------|--------------|
| 0 | Proto + regen | 30 min |
| 1A | Consul frontend | 4–6 hrs |
| 1B | Gateway verify | 15 min |
| 2 | Berthier overrides | 3–4 hrs |
| 3 | Integration + QA | 2–3 hrs |
| **Total** | | **10–14 hrs** |
