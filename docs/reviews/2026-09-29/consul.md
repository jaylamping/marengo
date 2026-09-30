# Consul operator frontend review

Reviewed on September 29, 2026. Scope covers the full operator frontend, telemetry transports/stores, command paths, commissioning UI, config/limits, teach/Auto Learn, charts, logs, simulation and URDF scaffolds. Findings use **main at `4bc77ba605834fdec04b436daa4bec67bca84fbb`** and one-based line numbers unless explicitly identified as older-branch behavior. Source links are pinned to that commit. Protobuf generation and production builds ran normally; no robot traffic was sent.

F21 is fixed in the review branch: route tests locate the parent route by its structure, and the primary check now runs the frontend suite. All other F findings remain unresolved. F05 overlaps G10 in the [gateway appendix](gateway.md). See the [finding index](finding-index.md) for complete status and the [repository review](../2026-09-29-repository-review.md) for later validation and completed cleanup.

## Verification

- Latest-main native Windows/Node 24 `npm run gen:proto`: passed.
- `npm run build`: passed TypeScript and Vite production build. Non-fatal warning: Telemetry page is both statically imported and dynamically prefetched, so prefetch cannot split that module.
- `npm test -- --run`: 69 files, 355 tests; 68 files/351 tests passed, 4 failures in [src/data/__tests__/hardware-commissioning-ia.test.ts](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/data/__tests__/hardware-commissioning-ia.test.ts). The test assumes `appRoutes[0]` is RootLayout, whereas a development preview route is prepended.
- Isolated mock reproductions: **13/13 passed**, demonstrating the observed defects rather than desired repaired behavior. They exercised the real frontend hooks/stores with fake timers and mocked network/renderer boundaries. The harness and outputs are retained locally with the migration recovery evidence; they never energized hardware.

The frontend results above describe the reviewed baseline. After the F21 repair, the native review-branch suite passes **355/355 tests**. Later container validation is recorded in the main repository report.

## Current architecture and working state

Consul is a Vite/React 19/TypeScript SPA, Radix UI primitives, Zustand runtime stores, TanStack Query for gateway metadata, Three.js for a Hardware schematic, and protobuf codegen for Chappe. A single app-level hook establishes WebTransport with HTTP length-prefixed stream fallback. State is throttled to about 10 Hz for UI; teach-record receives unthrottled RobotState samples. Gateway POSTs publish commands to Chappe. The Pi/Berthier/Davout stack remains the physical command/safety authority.

The initially recovered August 8 branch was substantially behind this main revision, so its old Actuators/Subsystems/preset workflow does not represent the reviewed architecture. Main has `/hardware` (commissioning scope, Verified/Active/fault facets, Set Limits/Set Zero, URDF import), `/telemetry` (read-only inventory), `/testing` (manual hold, PID, compound motions, teach, Auto Learn), `/logs`, and a wireframe Simulation route. Overview shows host/power cards and a CAN spectrum. Active description is master root YAML/URDF with a commissioning subset, rather than alternate bringup profiles. The shipped Wave now includes five right-arm joints and elbow-pitch oscillation. `WAVE_POSE_GCOMP_SIGNED` remains **false**: live Wave is intentionally uncommissioned at Start, but the run-time bypass below defeats that gate.

Live functionality is uneven. Host, joint, safety, limits, scope and log paths exist. Simulation controls are fixtures without RPC. Power readings are fixtures, labeled no-feed on Overview but still falsely healthy in inventory. Hardware 3D is an approximate static schematic, not the live master URDF or imported CAD meshes. Old FK/posture/chart components remain but are no longer mounted by latest Overview.

## Confirmed bugs and recommended fixes

### F01 — P1: Disable can be followed by an automatic live command and re-enable

**Evidence:** [consul/src/state/testingStore.ts:140–143](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/state/testingStore.ts#L140), [consul/src/hooks/use-compound-playback.ts:171–175,328–340,405–414](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/hooks/use-compound-playback.ts#L171). Disable awaits the POST, sets compound `isRunning=false`, but does not clear the hook-owned interval. The interval never checks `compound.isRunning`. The disconnect/operational-mode effect only stops when `isRunning` is true, so it becomes ineffective precisely after Disable clears that flag. Backend control reviewer verified that testing Position/Wave requests call `ensure_active_for_motion`, which can re-enable eligible motors.

**Trigger/consequence:** a running compound program is cancelled by Disable while a future waypoint/wave is pending; its timer later publishes the next live request, potentially re-energizing after the operator's explicit all-off action. Reproduced a post after the UI store was stopped and mode DISABLED.

**Fix:** one command owner must synchronously cancel intervals and invalidate all pending callbacks before Disable; each tick/post must verify a current run-generation and eligibility. Pi must require an explicit Enable after disable/stop rather than motion helper auto-enable. A backend command lease/cancel token should reject previously cancelled work.

**Test:** Disable/E-stop during raise, dwell, wave, pending POST and navigation; advance timers, resolve requests in reversed order, and assert no subsequent command or enable.

**Confidence:** high; frontend reproduction and backend confirmation.

**Latest-main backend anchors:** [bins/marengo-pi/src/main.rs:439–466,499–505](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-pi/src/main.rs#L439) drains testing commands into the motion helpers; [crates/berthier/src/loop.rs:421,465–482,504](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/berthier/src/loop.rs#L421) automatically invokes/resolves enable for motion. This is an operator-command lifecycle problem across UI and runtime, not a claim that a CAN driver skips Davout.

### F02 — P1: changing Dry Run while playing bypasses the Wave commission gate

**Evidence:** [consul/src/hooks/use-compound-playback.ts:207–223,322–325,409–411](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/hooks/use-compound-playback.ts#L207) and [consul/src/components/dashboard/testing/compound-test-panel.tsx](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/testing/compound-test-panel.tsx) Dry Run checkbox. Start checks signed-off status once using `dryRunNow`; subsequent dispatch reads the mutable global `dryRun`. The checkbox remains editable during playback.

**Trigger/consequence:** Start Wave in Dry Run, then uncheck Dry Run before its raise dwell finishes. A live `wave:...` command is sent even though `WAVE_POSE_GCOMP_SIGNED=false`; the dry raise was never commanded either. Reproduced on latest main.

**Fix:** freeze execution mode for the lifetime of a run; changing mode must stop/cancel and require a new Start through all gates. Check commission eligibility server-side as well.

**Test:** toggle both directions during every phase; a dry run must send zero motor commands, including cleanup/return-home.

**Confidence:** high.

### F03 — P1: Manual Hold Stop only updates the browser

**Evidence:** [consul/src/state/testingStore.ts:105](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/state/testingStore.ts#L105); mounted by [consul/src/components/dashboard/testing/hold-at-controls.tsx:38](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/testing/hold-at-controls.tsx#L38). `stopTest` only sets a local boolean. It neither cancels the Pi trajectory nor posts a mode/stop command.

**Trigger/consequence:** click Stop while a live hold trajectory is moving. UI changes to Start Hold but physical movement/hold remains latched. Backend reviewer confirmed Position target persists until another command, mode change or Disable. Reproduced zero outgoing requests on Stop.

**Fix:** implement a real acknowledged cancel/stop operation with defined safe behavior for supported/elevated arms. Display pending/stopped state from runtime. Avoid treating a local flag as motor completion.

**Test:** stop halfway through a simulated trajectory and verify runtime stops advancing under its documented policy.

**Confidence:** high.

### F04 — P1: changing a gain can retarget the joint to a different pose/mode

**Evidence:** [consul/src/state/testingStore.ts:151–173](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/state/testingStore.ts#L151); [consul/src/components/dashboard/testing/pid-slider-panel.tsx:62,75](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/testing/pid-slider-panel.tsx#L62); PID panel is also mounted under Compound Tests in [testing-overview.tsx](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/testing/testing-overview.tsx). Gain changes send an **IMPEDANCE motion batch** whose `position` is the manual store's shared `setpointRad`.

**Trigger/consequence:** select pitch manually (setpoint remains 0), start a raised compound pose, then change its PID slider in the compound panel. Tuning queues Impedance at 0 rad, replacing compound Position tracking instead of only adjusting gains. The request can also be issued when no manual test is running. Reproduction confirms the stale 0-rad target is sent.

**Fix:** use the dedicated `TuningChange` command path without changing control mode or target; serialize updates and acknowledge actual gains. Gate tuning against the current command owner.

**Test:** retune in Position, GravityComp, compound and idle states; assert target/mode remain unchanged and only the intended gain changes.

**Confidence:** high.

### F05 — P1: gateway connection is treated as fresh robot telemetry indefinitely

**Evidence:** [consul/src/hooks/use-chappe-telemetry.ts:91–120](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/hooks/use-chappe-telemetry.ts#L91); [consul/src/state/robotStore.ts](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/state/robotStore.ts) retains last RobotState/SafetyState/mode; Hardware's enable eligibility at [hardware-overview.tsx:48–53](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/hardware/hardware-overview.tsx#L48) does not require connection or freshness. Heartbeat handler is empty. Gateway reviewer confirmed its stream/cache remains alive without Pi and cached snapshots do not expire.

**Trigger/consequence:** marengo-pi stops/restarts or Chappe loses its producer while the gateway stays running. Consul can continue showing ACTIVE/Ready/live feedback from the last message and admit workflows against old data. A transport reconnect also leaves stale safety/mode until new data arrives.

**Fix:** track last-received timestamps and producer session/boot ID separately for robot/safety/heartbeat/host/limits. Mark stale/unknown using a timer, clear command eligibility and cancel runners. A gateway stream being open should mean transport connected, not robot healthy. Expire gateway cache snapshots too.

**Test:** stop Pi while keeping gateway connected; advance heartbeat TTL; all status and command gates must become stale. Restart with a new boot ID and verify old samples are not accepted.

**Confidence:** high; code plus gateway confirmation.

**Latest-main backend anchors:** [bins/marengo-gateway/src/state.rs:176–189,222–234](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/state.rs#L176) stores/decodes last RobotState/SafetyState/Heartbeat bytes without expiry; [http.rs:250–256](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/http.rs#L250) serves retained snapshots; [webtransport.rs:64–79](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/webtransport.rs#L64) subscribes to gateway envelopes without a producer freshness gate.

### F06 — P1: inventory presents fabricated safety/health statuses as machine truth

**Evidence:** [consul/src/data/robot-inventory.ts:122–123,136,155–159](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/data/robot-inventory.ts#L122); [consul/src/hooks/use-live-inventory.ts:14–35](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/hooks/use-live-inventory.ts#L14); [consul/src/components/dashboard/inventory/cells/inventory-status-cell.tsx](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/inventory/cells/inventory-status-cell.tsx) still renders the static status badge. Rows include E-stop `Nominal/closed`, BMS `48.1 V/87%`, an invented left ankle fault/position, and right motors Enabled. Live overlay only adjusts positions and can change Offline to Enabled; it ignores fault/drive state. New Reference badges do not replace the false primary status.

**Trigger/consequence:** inspect Telemetry with configured endpoints, including disconnected state; safety chain and power look nominal and motors can look Enabled while disabled/faulted. This conflicts with the Overview's more honest fixture labeling and known unconnected hardware E-stop input.

**Fix:** separate description catalog from telemetry facts. Unknown/no-feed/offline should be explicit; derive actuator status from current wire facets and drive-active state, never static fixtures. Only render demo fixtures inside an explicit demo mode.

**Test:** disabled, faulted, absent and stale joints; missing E-stop/BMS feeds; require honest unknown/offline labels.

**Confidence:** high.

### F07 — P1: the E-STOP button delays stop and silently discards failures

**Evidence:** [consul/src/components/dashboard/testing/e-stop-button.tsx:9–16](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/testing/e-stop-button.tsx#L9). First click only opens a three-second confirmation; actual Disable needs a second click. The promise is not awaited/caught and UI resets immediately. This is a software Disable request, not a physical E-stop or latched stop.

**Trigger/consequence:** an operator presses the prominently labeled E-STOP once expecting motion interruption; no request is sent. Network/backend rejection on the second click has no visible feedback.

**Fix:** send the software stop on first activation, with pending/acknowledged/failed state and clear semantics. Keep physical E-stop independent; a dedicated runtime latch prevents subsequent motion from undoing the stop. Make stop accessible from every operator route.

**Test:** one click must dispatch exactly one stop; request rejection must be visible; subsequent motion must not clear the latch without reset/Enable.

**Confidence:** high.

### F08 — P2: transport reader failures skip disconnect and reconnection

**Evidence:** [consul/src/lib/chappe-transport.ts:222–247,280–301](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/lib/chappe-transport.ts#L222). Both detached async loops await `readLengthPrefixedFromStream` outside any outer catch/finally. Only envelope decode is caught. An errored stream rejects `reader.read()`, bypassing `onDisconnected` and thus the reconnect scheduler in `chappe-client.ts`.

**Trigger/consequence:** QUIC/session error or HTTP stream network failure; frontend can retain connected state and freeze until a full page reload.

**Fix:** wrap the complete reader lifetime in try/catch/finally; report the error, dispose transport and signal disconnect once unless intentionally closed. Close partially-created WT sessions if handshake/subscription fails. Give WT establishment a bounded application timeout so reachable HTTP fallback is attempted promptly.

**Test:** force `reader.read()` rejection, EOF, malformed frames, WT handshake and subscription errors; assert recovery and no leaked session/unhandled rejection.

**Confidence:** high; direct rejection control flow, no hardware needed.

### F09 — P2: HTTP stream cleanup never aborts its network/reader

**Evidence:** [consul/src/lib/chappe-transport.ts:262–264,275–278,303](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/lib/chappe-transport.ts#L262). AbortController is created after fetch and its signal is never passed to fetch. Cleanup merely sets its local flag; pending `reader.read()` is not cancelled. Closing before setup returns also leaves `res.body` uncancelled.

**Trigger/consequence:** unmount/reconnect during a quiet HTTP stream; old connection/reader remains waiting, consuming server/client resources and potentially delivering a late frame into stale handlers. Mock reproduction confirms no fetch signal and no reader cancellation.

**Fix:** create controller before fetch, pass signal, cancel/release reader in cleanup/finally, and return a cancellation handle before waiting indefinitely for establishment.

**Test:** no-data stream cleanup must settle immediately and invoke cancellation; repeating reconnect must leave only one reader/socket.

**Confidence:** high.

### F10 — P2: StrictMode leaks the first asynchronously established telemetry subscription

**Evidence:** [consul/src/hooks/use-chappe-telemetry.ts:69–77,147–159](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/hooks/use-chappe-telemetry.ts#L69); [consul/src/main.tsx](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/main.tsx) enables root StrictMode. First effect cleans up while connection is pending, then second setup flips the shared ref back to false. First promise later resolves and retains its disposable in an effect instance whose cleanup already ran.

**Trigger/consequence:** normal development mount; two network subscriptions can survive and the first remains after unmount, creating duplicate samples and state changes. Reproduced using root StrictMode, two pending connection promises, and final unmount: only second disposal runs.

**Fix:** effect-local `let disposed` or monotonically increasing generation; cancel trailing telemetry throttles on cleanup too.

**Test:** resolve both StrictMode connection promises after the initial cleanup; exactly one active subscription remains, then zero on unmount.

**Confidence:** high.

### F11 — P2: native Wave Loop repeats only the UI countdown

**Evidence:** [consul/src/hooks/use-compound-playback.ts:355–366](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/hooks/use-compound-playback.ts#L355). Loop expiration changes timestamps and returns without posting another native wave. Wire command specifies a finite 50 cycles × 2 × 1.4 seconds = 140 seconds at speed 1. Backend reviewer confirmed finite wave finishes and holds its final min.

**Trigger/consequence:** keep Loop checked for more than one wave duration; physical oscillation stops while the browser claims to keep running/looping. Reproduced no second POST after 145 seconds.

**Fix:** put continuous loop ownership on Pi with explicit cancellation, or re-arm at an acknowledged completion boundary. Report runtime phase/completion rather than estimating from wall time.

**Test:** run beyond two cycle windows; commanded/runtime wave and UI phase must agree; speed changes must not silently change only UI timing.

**Confidence:** high.

**Latest-main finite-wave anchors:** [crates/berthier/src/position_wave.rs:44–50](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/berthier/src/position_wave.rs#L44) finishes at the finite total ticks; [crates/berthier/src/position_hold.rs:619–628](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/berthier/src/position_hold.rs#L619) sets the final endpoint and clears the wave.

### F12 — P2: non-timed dry-run programs never finish

**Evidence:** [consul/src/hooks/use-compound-playback.ts:333–394](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/hooks/use-compound-playback.ts#L333); default settle presets in [consul/src/data/compound-tests.ts](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/data/compound-tests.ts). Dry Run suppresses commands but still waits for real measured position/velocity. It does not generate a preview pose.

**Trigger/consequence:** Dry Run Arm Fully Up/Arm Out Forward without a robot already at the target; progress reaches 100% but isRunning stays true forever. Auto Learn's default Test Proposal is similarly not a meaningful physical preview/validation for these presets.

**Fix:** a dry executor should advance an explicit simulated/preview timeline without real settle gates. Show separate estimated preview and live state. Live executor retains measured settle checks and bounded timeout/failure.

**Test:** every shipped preset completes a dry run with zero network commands and no RobotState.

**Confidence:** high; reproduction.

### F13 — P2: teach-record cannot extract the new elbow Wave

**Evidence:** [consul/src/lib/teach-record.ts:145–175](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/lib/teach-record.ts#L145) only searches shoulder roll and upper-arm yaw extrema; main Wave at [consul/src/data/compound-tests.ts](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/data/compound-tests.ts) now oscillates elbow pitch and documents elbow extrema.

**Trigger/consequence:** support the commissioned raise pose and teach elbow-pitch nods while other joints remain steady; extractor returns just an upright landmark, so Apply refuses despite clear motion. Reproduced 600 samples of elbow oscillation with one landmark.

**Fix:** make extraction capability/preset-driven and include elbow/lower-arm/selected moving DOFs, preserving synchronized full-pose snapshots. Avoid hard-coded named joints for generic teach programs.

**Test:** elbow-only Wave, yaw Wave, lower-arm motion and single-joint programs yield sufficient appropriate landmarks without fabricated cross-joint poses.

**Confidence:** high.

### F14 — P2: actual Set Zero does not invalidate taught coordinates

**Evidence:** [consul/src/components/dashboard/inventory/set-limits-panel.tsx:198–200](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/inventory/set-limits-panel.tsx#L198) never calls the calibration epoch update. Only `manual-movement-panel.tsx:35–36,140` exposes a separate manual `I set-zero'd` action. No wire calibration/zero registry revision is included in TeachFingerprint.

**Trigger/consequence:** apply an overlay, Set Zero that joint via Hardware or MCP, then replay without separately remembering the manual calibration button; old numeric poses still pass fingerprint/epoch checks against a shifted mechanical zero.

**Fix:** publish authoritative calibration revision per joint/robot and record it in teach session. Dirty overlays immediately when a zero request starts, then reconcile acknowledged verification. Refuse replay on mismatch until explicit review.

**Test:** Set Zero inside/outside UI, restart with changed registry, and another tab's calibration; stale overlay is blocked automatically.

**Confidence:** high for missing automatic invalidation; impact depends on size of zero shift.

### F15 — P2: live Hardware 3D rebuilds the WebGL scene at telemetry rate

**Evidence:** [hardware-overview.tsx:84–85,96–102](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/hardware/hardware-overview.tsx#L84) creates new rows for each RobotState; [hardware-3d-view.tsx:31–42](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/hardware/hardware-3d-view.tsx#L31) disposes/reconstructs `BenchArmScene` when rows identity changes. Selected effect at `:44–46` only depends on selectedJoint.

**Trigger/consequence:** switch to 3D while telemetry arrives around 10 Hz; camera/orbit resets continually, contexts/resources churn, and current selection is lost after rebuild. Reproduction with equivalent new row array constructs a second scene, disposes first, and does not reapply selection.

**Fix:** initialize renderer/scene once; update status/rows imperatively, adding/removing joints only when topology changes, and preserve/reapply selection/camera. The prototype PR109's stable scene lifetime is useful as a pattern, without importing its mock semantics.

**Test:** update thousands of joint samples and limit polls; one scene/context, persistent camera and selection.

**Confidence:** high.

### F16 — P2: a local Range override permanently masks later live limit truth

**Evidence:** [consul/src/components/dashboard/hardware/hardware-overview.tsx:43,96–102,227–230](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/hardware/hardware-overview.tsx#L43). Apply stores a display string in `rangeOverrides`; it is always preferred over `row.liveRange` and never cleared by newer snapshots/import/restart.

**Trigger/consequence:** apply limits then update those limits through MCP, another browser, import or reload within the same mounted Hardware page; displayed 'Live hard range' remains the earlier local range while disk/live data changed.

**Fix:** use the acknowledged snapshot as SoT; if optimistic display is needed, tie it to a pending request/revision and clear it once matching/newer live snapshot arrives.

**Test:** Apply A, receive B later from another source; UI must display B, including open sheet.

**Confidence:** high.

### F17 — P2: live ERROR/WARN logs can be dropped before severity is known

**Evidence:** [consul/src/lib/log-buffer.ts:13,310–323](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/lib/log-buffer.ts#L13); `chappe-transport.ts` calls shouldDecodeLogEvents before decoding LogEvent. Its unconditional 12/s cap defeats the documented warn/error bypass in allowIngest.

**Trigger/consequence:** 12 ordinary log envelopes arrive first in a second, then a motor/safety ERROR; critical message is silently omitted from live UI. Reproduced dropping the ERROR callback after 12 decode slots. Archived server logs may still retain it.

**Fix:** apply severity-aware sampling after decoding, or use separate severity channels/bounded queues, and expose dropped-event counts. Never discard stop/fault events behind routine-info traffic.

**Test:** saturate INFO/DEBUG then send WARN/ERROR/FATAL; all critical events remain visible.

**Confidence:** high.

### F18 — P2: archive requests race and can show the wrong session/page

**Evidence:** [consul/src/hooks/use-archive-sessions.ts:75–90](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/hooks/use-archive-sessions.ts#L75) and [consul/src/hooks/use-candump-data.ts:46–83](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/hooks/use-candump-data.ts#L46) attach responses without generation/cancellation checks.

**Trigger/consequence:** select slow session A, then fast B, or change CAN page; A resolves last and replaces data under B's selection/offset. Fault analysis can be attributed to the wrong run.

**Fix:** keyed TanStack queries with abort signals, or effect-local generation checks and cleanup. Reset offset when session/view changes and tie summary and page to the same selected session.

**Test:** resolve A/B/page requests out of order; rendered data must always correspond to selected key.

**Confidence:** high.

### F19 — P2: archive UI silently truncates or filters only the first page

**Evidence:** [use-archive-sessions.ts:82–83](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/hooks/use-archive-sessions.ts#L82) always requests first 500 bench/trace lines and ignores total; no next/tail control. [logs-archive-search.tsx:79,91](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/logs/logs-archive-search.tsx#L79) requests first 200 matches then applies source filter locally.

**Trigger/consequence:** fault near end of long run cannot be viewed; Davout source search can show zero results when matching Davout rows exist after the first 200 other results. Total shown is the unfiltered server total.

**Fix:** server-side source filter, pagination/cursor/tail with accurate totals, explicit truncation indicators, and useful default last-lines view.

**Test:** match/fault after row 500/200 and verify it is reachable/searchable.

**Confidence:** high.

### F20 — P2: self-update can declare success for another job's installed target

**Evidence:** [consul/src/components/dashboard/sidebar/use-sidebar-self-update.ts:49–57,70–83](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/sidebar/use-sidebar-self-update.ts#L49). installedOnWatchTarget uses **either** the watched session target or status.deploy.target_sha; its success path is outside sameJob.

**Trigger/consequence:** browser watches new job target B, gateway reports old/different job whose target A matches currently installed A. Watch resolves success and reloads before B is installed. Reproduced helper input with job IDs old/new and targets A/B.

**Fix:** prioritize session.targetSha when present; only use ledger target when job IDs match and session target absent. Require intended target + coherent readiness, without conflating installed old target with successful update.

**Test:** stale old ledger, different concurrent job, missing target, installed watched target and failure of same job.

**Confidence:** high for incorrect helper result; trigger requires stale/different ledger snapshot.

### F21 — P2: latest-main UI suite has four failing route tests

**Status:** Fixed in the review branch. The four route failures are repaired by locating the route with children instead of assuming index zero; native tests pass 355/355. The primary gate now invokes the frontend suite. The evidence below documents the original baseline defect.

**Evidence:** [consul/src/data/__tests__/hardware-commissioning-ia.test.ts:12–18](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/data/__tests__/hardware-commissioning-ia.test.ts#L12) assumes index zero; [consul/src/routes/config.tsx:19–28](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/routes/config.tsx#L19) prepends dev route. Actual native Vitest failures listed above.

**Fix:** find the route containing RootLayout/children using stable route identity rather than array position; verify both DEV/PROD surfaces. Preserve route behavior.

**Test:** the existing four cases plus preview route registration.

**Confidence:** high; execution.

### F22 — P2: durable limit save can hang/fail at optional local-sync after it already succeeded

**Evidence:** [consul/src/lib/persist-joint-limits.ts:123–136,166–189](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/lib/persist-joint-limits.ts#L123). Pi patch has a 30-second abort, but optional local-sync fetch has no deadline and is awaited before returning success. UI receives neither refresh nor success while localhost service stalls. Exceptions from an injected localSync can also make a durable Pi save look wholly failed.

**Fix:** report Pi Durable separately and refresh live UI immediately; run bounded/retryable local sync as a distinct status. Pass actual active profile identity rather than relying on default 'master' on every caller.

**Test:** localhost stalls/rejects after Pi Durable; frontend promptly reports durable robot result and local-sync failure separately.

**Confidence:** high.

### F23 — P2: Set Zero shows Applied for a queued command with no verification ACK

**Evidence:** [consul/src/components/dashboard/inventory/set-limits-panel.tsx:71,198–200,305–313](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/inventory/set-limits-panel.tsx#L71) turns the same green Applied flash on for successful queue POST. Its own comment says HTTP 200 means queued only; there is no corresponding zero-verified correlation/error observation.

**Trigger/consequence:** Pi refuses/fails/disappears after gateway accepts publication; UI flashes Applied even though mechanical reference was not verified. Reference badge may remain Not ready, giving contradictory feedback. Local older checkout said 'queued'; main lost that honesty.

**Fix:** show queued/pending until matching runtime verification (joint/request/boot identity) arrives; show refusal/timeout and only call applied when verified. Tie calibration invalidation to this protocol.

**Test:** queue success + runtime refusal/timeout and genuine Verified completion.

**Confidence:** high.

### F24 — P3: deprecated chart time controls reject all actual live timestamps

**Evidence:** [use-chappe-telemetry.ts:32](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/hooks/use-chappe-telemetry.ts#L32) writes a decimal millisecond remainder string; [components/dashboard/charts/utils.ts:11–21](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/charts/utils.ts#L11) expects `mm:ss` and compares hard-coded demo timestamps. History also caps at 120 points (~12 seconds), not 1/5 minutes. Mock test confirms both live range filters empty.

**Fix:** numeric monotonic/full timestamp series and actual rolling elapsed-time filtering with appropriate ring buffer/downsampling. Current latest Overview no longer mounts this card, so this is dormant code debt, not a current visible-route outage. Older migrated checkout still mounts it.

**Confidence:** high.

### F25 — P3: archive listing fetch runs as a render side effect

**Evidence:** [consul/src/components/dashboard/hardware/import-wizard.tsx:348–353](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/hardware/import-wizard.tsx#L348) calls fetchUrdfArchiveList while `loaded=false` during render.

**Trigger/consequence:** StrictMode/parent rerenders before first response cause duplicate requests and late setState after close/reopen; cancelled work can repopulate a different import session.

**Fix:** keyed query/effect with cleanup/session generation; guard async upload/resolve responses after wizard close/reset too.

**Test:** render repeatedly with delayed response, close/reopen during upload; one current fetch/result, no stale wizard state.

**Confidence:** high.

## Architecture/design debt and incomplete features

1. **Command orchestration is distributed across UI state, hook timers and raw batch POSTs.** Manual hold, gain changes, compound, Home and external clients can compete without explicit ownership. Local isRunning/returnHomePending values are not runtime motion state. Return Home 'settled' is a fixed 2.5-second sleep ([use-compound-playback.ts:140](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/hooks/use-compound-playback.ts#L140)), and route unmount cancels browser bookkeeping without cancelling a Pi-native wave. Move execution/lease/phase/cancel/ack ownership to the Pi; frontend requests programs and reflects runtime truth. Add typed command IDs, ordered generation and stop barriers.
2. **Publish acknowledgement and action completion differ but several UIs conflate them.** Generic Enable, zero, tuning and motion helpers mostly return HTTP publish success. A shared response/ACK adapter should expose queued, admitted, executing, completed/refused and persistence stages. Native failure is otherwise only detectable through unrelated logs/state.
3. **Safety semantics need one contract.** The prominent button says E-STOP but posts ordinary Disable; inline 'Stop' can mean no-op or return to home. Define stop/cancel/disable/emergency-latch separately, with required GravityComp/support behavior, wire them to matching runtime capabilities, and make critical controls persistent across routes.
4. **Hardware facts are duplicated in frontend constants.** `MASTER_LIMBS`, wired joint allowlist, static limits/gain tables, preset joints and profile maps require code updates when CAD/config changes. Gateway should expose validated topology/limb/motor-capability manifest plus its revision. Use the manifest to render and gate all joints. Hard-code only demo fixtures.
5. **Frontend credentials are baked into downloaded JavaScript.** `VITE_MARENGO_LOG_TOKEN` and `VITE_AUTO_LEARN_TOKEN` are public to anyone who can retrieve the SPA; never treat them as protected server secrets. Pair with gateway/BFF access control design. G07 in the [gateway appendix](gateway.md) covers the serving/CORS/auth boundary and distinguishes conditional build-time exposure from observed secret data.
6. **Control mode/calibration/gains/setpoint/run phase are not authoritative wire telemetry.** Teach relies on a checkbox declaring GravityComp; actual robot mode can be changed by REPL/MCP/other tab without invalidating it. Fingerprint uses deploy SHA/profile/joint names, not URDF/config/calibration revisions. Add these runtime facts for truthful capture/preflight/replay.
7. **Hardware 3D is a static approximate schematic.** It ignores master URDF, imported CAD meshes, actual FK and joint movement. Import wizard currently renders only kinematics-critical conflicts; it does not summarize new joints, noncritical changes or all gravity-relevant mass/COM effects before activation. Show a complete proposal and effective active/needs-restart state, and synchronize actual model revisions.
8. **Dormant URDF/FK implementation should be fixed before reuse.** [parse-urdf.ts](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/urdf/parse-urdf.ts) supports only box/cylinder/sphere, so actual SolidWorks meshes fail; default joint axis is zero rather than standard X; getRobotModel globally caches first XML regardless of later argument. [forward-kinematics.ts](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/urdf/forward-kinematics.ts) treats every joint as rotational (including prismatic), ignores mimic, and uses THREE XYZ origin Euler order rather than URDF fixed-axis roll/pitch/yaw composition; [urdf-scene.tsx](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/urdf-preview/urdf-scene.tsx) duplicates the same origin convention. Current live Hardware schematic does not use these modules, so no present motor risk is claimed.
9. **Simulation and product stubs should be obvious and disabled until functional.** Simulation uses fixture session/events/scenarios and no-op Play/Pause/Stop/Step controls, with small wireframe hint. Older checkout has enabled Calibration/Recover fault/Go/Sweep stubs; latest Telemetry removed the motion stubs. No deployed learned planner/simulation bridge should be inferred from polished controls. Power fixture metrics remain without real feed.
10. **Missing behavioral integration coverage.** Existing 355 tests predominantly validate helpers, UI shells and individual admission conditions; they do not cover command cancellation/lifecycle, transport rejection/reconnect, mid-run mode changes, native runtime completion or an end-to-end no-hardware gateway/Pi simulator. The 13 outside-repo reproductions are candidate seeds for proper fix regression tests (reverse assertions when fixing).
11. **Performance/data-retention strategy is inconsistent.** Teach copies the entire growing sample array on every 50 Hz update (up to 50,000 samples), UI telemetry commits one Zustand update per joint, and canvas/status routines poll stale values as though new observations arrived. Use bounded sample/ring buffers, batched robot updates, explicit source timestamps and a shared time cursor.
12. **Client API schema validation/error handling is uneven.** Some Hardware endpoints validate response shape; Config/log/Auto Learn clients often cast JSON, and many hooks swallow failures/null responses or leave unhandled POST rejections. Consolidate endpoints, authentication, cancellation/timeouts and typed errors without introducing duplicated protocol shapes.

## Findings already fixed by updating to main

Do not re-file these old-checkout defects as unfixed main bugs: persisted browser actuator-zero flags/Testing bulk Home readiness; obsolete Actuators page; bringup preset editing in Telemetry; UI Set Limits adds ±30 mrad to persisted hard limits; implicit localhost limit-sync fallback; stale persisted config snapshot restoration; hard-coded displayed CAN IDs not using disk map. Main also changes pitch/roll CAN IDs and adds lower-arm yaw, so old bench assumptions and recovered CAD manifests still need reconciliation against the current master before physical work.

## PR #109 / origin/jl/hardware-proto-threejs-5293

The draft has seven unique commits and approximately 1,938 added lines across 16 files. Novel material is the throwaway A/B/C Hardware prototypes, Bender-like humanoid schematic, mock fixtures and DEV-only route/launcher. Production main already implements the selected table-first Hardware + optional 3D + settings/import direction with live APIs. The richer prototype renderer's stable scene lifetime is a useful design reference; it does not provide missing live hardware functionality.

Recommendation: preserve it as a design archive/superseded draft, or salvage only the prototype directory behind a DEV route if intentionally desired. Do not promote mock control/health semantics into production. Full merge requires reconciling CONTEXT, sidebar and route conflicts with the newer Hardware/Telemetry commissioning IA. The prototype's 'Accept = Active in-memory' glossary disagrees with the current URDF activation restart-required behavior and must not override current safety/documentation. A merge should not replace newer master YAML/URDF, commissioning, deploy or limit-persistence code merely because the old branch has those older versions.

## Suggested implementation order

1. Fix F01–F07 and enforce explicit Enable/stop barriers in Pi, plus trusted freshness/true-status contract.
2. Introduce a shared acknowledged command/run controller; fix gain-only updates and stale/cancelled work.
3. Fix telemetry resource lifetime/reconnect (F08–F10), native playback/dry-run/teach (F11–F14), and priority log ingestion (F17).
4. F21 is repaired in the review branch, including running frontend tests in the primary gate. Next fix Hardware scene/live-range, archive correctness, update target matching and zero ACK feedback.
5. Remove/delimit fixtures and retired code; generate topology/capabilities from config; build live URDF/CAD and simulation features when required by commissioning.

## PR #107 — older Config/Setup hydration glossary

The `b807f8e` glossary patch should be **updated before any merge**, because its intended architecture differs from what latest main implemented:

| Older proposed term/claim | Actual latest-main implementation | Required wording |
|---|---|---|
| Config/Setup owns a multi-assembly Pi URDF library and companion profile YAML | Hardware owns one master `assets/urdf/marengo.urdf` + root master YAML; profiles are retired from operator SoT | Hardware / master description; archive entries are contributors/history rather than independently active limb assemblies |
| Make Active selects a library assembly/profile | Upload stages a contributor blob, resolver picks critical joint-field winners, Accept promotes the **merged master URDF** | Upload → Resolve → promote merged master; no active profile/assembly selector is implemented |
| Accept applies into runtime memory immediately and writes behind | [bins/marengo-gateway/src/hardware.rs:314–384](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/hardware.rs#L314) simulates a field-resolved merge, archives contributor/replaced master/manifest, writes/renames master XML on disk, and returns `restart_required=true`; it does not tell Pi to reload kinematics/dynamics | Durable URDF promotion succeeded; restart Pi while disabled and re-verify reference/model before motion |
| Persist-degraded Accept has new model active in memory even if disk failed | URDF activation currently writes disk synchronously; errors return failed activation (archive may already exist). Runtime continues old loaded model until restart. Numerical **Set Limits** does have separate live/write-behind Pending/Durable/Failed contract | Distinguish URDF activation from numerical limit hot-reload; do not transfer ADR 0012 semantics to URDF until implemented |
| Updates available is per library/assembly/companion-YAML pending chrome | Current wizard displays critical conflicts and archive restore; no persistent Updates-available dashboard or parallel YAML staging exists | Keep this as unimplemented future requirement, not present behavior |
| Dismiss deletes staged entry | UI Cancel resets browser wizard; no staging-delete endpoint is called | Cancel leaves master unchanged; staged files need explicit expiry/delete cleanup if desired |

The implementation **does contain real field-level resolution**, rather than blindly activating an uploaded blob: `post_resolve_preview` at [hardware.rs:230–256](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/hardware.rs#L230) and `simulate_merge_xml` at [crates/marengo-config/src/urdf_merge.rs:227–251](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-config/src/urdf_merge.rs#L227) validate the merge and explicit critical choices. Staging/archives are blob directories with manifests, and restore stages a contributor for that same merge flow. These two levels should both be described accurately.

The current UI acknowledges the runtime mismatch at [consul/src/components/dashboard/hardware/import-wizard.tsx:151,305–307](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/hardware/import-wizard.tsx#L151), saying Pi must restart before new URDF is enforced, but its `Accept → Active` label and several OpenSpec/Context claims of immediate runtime activation are still misleading. Preserve the latest commissioning/glossary work and replace only the stale hydration vocabulary; do not merge the older multi-profile assumptions as current facts.
