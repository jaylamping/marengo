# ADR 0014: Jetson perception + semantic-to-motion pipeline

**Status:** Accepted  
**Date:** 2026-06-19

## Context

The robot is currently Pi-only: `marengo-pi` owns real-time control (CAN, IMU, Berthier → Davout → robstride) and `marengo-gateway` bridges Chappe to Consul. The architecture diagram ([architecture.md](../architecture.md)) already shows `Jetson → Chappe` and `Fouché → Chappe`, and the workspace ships a scaffolded `bins/marengo-jetson` (6-line `main`) plus a 1-line `crates/fouche`. Nothing is wired.

A Jetson Orin Nano (8 GB) is being added to the robot. The motivating use case is **natural-language commands** — e.g. "wave your right hand" — where the robot decomposes the semantics into a motion plan at runtime rather than replaying a preprogrammed loop. This ADR captures the integration architecture and the semantic→motion pipeline design so future work (and future-forgetful Joey) has one reference.

## Decision

### 1. Role split — Jetson is perception + intent, never control

| Node | Owns | Must not |
|------|------|---------|
| **Pi** | Real-time control: CAN, IMU, Berthier → Davout → robstride, gateway | Vision inference, LLM inference |
| **Jetson** | Vision (MIPI CSI + NPU), semantic→intent decomposition (LLM client), host metrics for the Jetson node | CAN, motors, `davout`, `robstride`, real-time control loops |
| **Off-robot LLM host** | LLM inference (3070 Ti PC local, or OpenRouter remote) | Anything on the robot |

Jetson stays strictly above Chappe. This preserves the existing crate boundaries ([rust-patterns.md](../rust-patterns.md) §3, §7): no new caller of `davout` or `robstride`, no new CAN participant. Berthier and Davout are unchanged.

### 2. Physical wiring

- **Power:** USB-C PD (≥45 W) or barrel jack on the Orin Nano devkit.
- **Network to Pi:** Dedicated Ethernet on an isolated subnet (e.g. Pi `10.10.0.1`, Jetson `10.10.0.2`). Do not route through the house LAN — keep the link deterministic. This is the Chappe transport.
- **Cameras:** MIPI CSI to the **Jetson**, not the Pi. The NPU/GPU is on the Jetson; CSI on the Pi would waste both.
- **CAN:** stays on the Pi's `can0` / `can1`. No CAN wire to the Jetson.
- **IMU (BNO085):** stays on the Pi's I2C. The control loop needs it at tick rate and it is torso-mounted.
- **Mechanical:** Jetson lives on a shelf TBD (separate workstream).

### 3. Chappe network transport (the missing architectural piece)

Chappe is currently in-process broadcast + Unix socket IPC to the gateway (ADR 0008). There is **no network transport between Pi and Jetson** yet. `config/network.yaml` has a placeholder `chappe_bind: "0.0.0.0:7447"` but no bridge implementation.

**Decision:** add a TCP bridge in `chappe::transport::net` — a `TcpBridge` that fans `Envelope` bytes both directions with length-prefix framing (same framing as the gateway HTTP stream fallback in ADR 0008). This matches ADR 0001 (binary protobuf on the wire), is a few hundred lines, and is swappable for Zenoh later without touching publishers.

**Why not Zenoh / NATS now:** only two nodes (Pi + Jetson) on a dedicated subnet. TCP is sufficient and avoids a new dependency surface. ADR 0008 already notes a future `--transport nats` mode; the same swap pattern applies here.

**Why not reuse the gateway WebTransport:** couples Jetson availability to the gateway being up; Jetson ↔ Pi should work headless.

### 4. Semantic → motion pipeline

```
"Wave your right hand"
        ↓  [Jetson / fouche::intent]   LLM decomposition
   MotionSpec { phases: [MotionPhase; …] }
        ↓  [Chappe: navigator/intent]
   Talleyrand
        ↓
   IK per phase → joint trajectory with timing
        ↓  [in-process on Pi]
   Berthier joint-space executor (unchanged)
        ↓
   Davout → robstride → motors
```

**Critical design rule: the LLM emits a structured motion spec, not raw Cartesian waypoints.** LLMs are unreliable at exact 3D coordinates (hands inside the torso, unreachable poses, no notion of timing/smoothness). The LLM's job is **semantic decomposition** ("what is a wave? → raise + oscillate + lower") and **primitive selection + parameterization** from a fixed catalog. Geometry is Talleyrand's IK.

This is the same pattern as RT-2 / Code as Policies / SayCan: LLM picks from affordances, a classical layer executes.

### 5. Motion primitive + labeled-pose catalogs

Two catalogs live in a crate (`fouche::primitives` or `talleyrand::primitives`, TBD) and are given to the LLM as tools/affordances:

- **Motion primitives:** `reach_to`, `oscillate`, `return_to`, `hold`, `point`, `grasp_preshape`, … (10–20 entries; the real intellectual work, needs no hardware).
- **Labeled poses:** `home`, `shoulder_height_side`, `arm_extended_forward`, … — frame-relative named poses Talleyrand resolves against the URDF `base_link`.

The LLM emits `MotionPhase` sequences referencing these by name; Talleyrand resolves labels to `CartesianPose`, runs IK per phase, emits joint trajectories to Berthier.

### 6. Proto additions (proto-first, per AGENTS.md)

Add to [`proto/marengo/v1/marengo.proto`](../../proto/marengo/v1/marengo.proto):

```protobuf
message CartesianPose {
  double x = 1; double y = 2; double z = 3;
  double roll = 4; double pitch = 5; double yaw = 6;
  string frame = 7;  // e.g. "base_link"
}

enum MotionPrimitive {
  MOTION_PRIMITIVE_UNSPECIFIED = 0;
  MOTION_PRIMITIVE_REACH_TO = 1;
  MOTION_PRIMITIVE_OSCILLATE = 2;
  MOTION_PRIMITIVE_RETURN_TO = 3;
  MOTION_PRIMITIVE_HOLD = 4;
}

message MotionPhase {
  MotionPrimitive primitive = 1;
  string target_label = 2;        // semantic: "shoulder_height_side", "home"
  CartesianPose target_pose = 3;  // optional: explicit pose when provided
  string axis = 4;
  double amplitude_rad = 5;
  uint32 cycles = 6;
  double period_s = 7;
  double duration_s = 8;
}

message NavigatorIntent {
  uint64 timestamp_ms = 1;
  string utterance = 2;           // original command, for logging/replay
  string arm = 3;                 // "left" | "right"
  repeated MotionPhase phases = 4;
  string source_node = 5;         // "jetson"
}

message DetectedObject {
  string label = 1;
  double confidence = 2;
  // 2D bbox in image space; 3D pose optional
  double bbox_x = 3; double bbox_y = 4; double bbox_w = 5; double bbox_h = 6;
  CartesianPose pose_3d = 7;      // optional, in base_link
}

message PerceptionFrame {
  uint64 timestamp_ms = 1;
  string frame_id = 2;            // camera id
  repeated DetectedObject detections = 3;
  string source_node = 4;         // "jetson"
}
```

Regenerate with `cd consul && npm run gen:proto` + `cargo build -p armee-proto`. Do not hand-edit `consul/src/gen/`.

### 7. Topics

| Topic | Publisher | Subscriber | Payload |
|-------|-----------|------------|---------|
| `heartbeat/jetson` | `marengo-jetson` | gateway / Consul | `Heartbeat` (`node_id="jetson"`) |
| `host/metrics/jetson` | `marengo-jetson` | gateway / Consul | `HostMetrics` with `JetsonPlatformMetrics` |
| `perception/frame` | `fouche` | Talleyrand (optional, scene context) | `PerceptionFrame` |
| `navigator/intent` | `fouche::intent` | Talleyrand | `NavigatorIntent` |
| `robot/state` | `marengo-pi` | `marengo-jetson` (optional, for perception context) | `RobotState` |

`JetsonPlatformMetrics` and `HOST_NODE_ROLE_JETSON` already exist in the proto (lines 283–294, 148–152) — host telemetry is wire-ready.

### 8. Where the LLM runs — memory math drives the split

Jetson Orin Nano 8 GB is **shared** CPU/GPU memory. Approximate budget:

| Workload | Memory |
|----------|--------|
| OS + CUDA context | ~1 GB |
| YOLO-v8n INT8 (TensorRT) | ~300–500 MB |
| Camera capture buffers | ~100–300 MB |
| 7B LLM INT4 (llama.cpp) | ~4 GB |
| 3B LLM INT4 | ~2 GB |
| Rust binaries + Chappe | ~200 MB |

**Vision + a 7B LLM on the same 8 GB Jetson is the failure mode** — they fight for memory and the LLM thrashes the GPU while YOLO tries to hit 30 fps. The Jetson NPU is excellent at INT8 vision (~68 GB/s bandwidth) but mediocre at LLM (10× slower than a discrete GPU).

**Decision: Jetson does vision only. LLM lives off the robot.**

| Layer | Host | Rationale |
|-------|------|-----------|
| Vision / perception | Jetson | MIPI-attached; NPU runs YOLO at 30–60 fps INT8; continuous workload belongs on edge |
| LLM semantic decomposition | 3070 Ti PC (local) **or** OpenRouter (remote) | Bursty; needs quality; 3070 Ti = ~25–40 tok/s on 7B Q4; OpenRouter Claude Opus 4.x gives best "what is a wave" reasoning |
| Control / IK / motors | Pi | real-time, unchanged |

**`fouche::intent` exposes a trait `IntentDecomposer` with two impls:**
- `OpenRouterClient` (HTTP via `reqwest`)
- `LocalLlamaCpp` (subprocess or HTTP to a local `llama-server` on the PC)

Toggled via config. **Default to OpenRouter initially** — reasoning quality matters most during prompt-design; switch to local 3070 Ti once the prompt + primitive catalog are stable and offline/low-latency matters.

### 9. Vision vs. action concurrency

For "wave your hand," **vision is not in the loop during execution** — the LLM emits a `MotionPhase` sequence and Berthier executes open-loop. Vision is for:
- **Scene priming:** "person 2 m in front" → informs which actions make sense
- **Verification:** post-wave, check the arm moved and nothing was hit
- **Future closed-loop tasks:** grasping, where hand→object tracking matters

So for the wave use case, **vision and the LLM never run simultaneously.** Vision publishes `PerceptionFrame` at low rate (5–10 Hz is plenty for scene context); the LLM fires only on user command. This further reduces Jetson memory pressure — if local-LLM-on-Jetson ever becomes necessary, vision can be paused briefly during LLM inference.

### 10. `marengo-jetson` bin — flesh out the scaffold

Replace the 6-line `main.rs` with the same shape as `marengo-pi`:
- `chappe::tracing_layer::init_subscriber(Some(chappe_bus), "marengo-jetson")` (Chappe producer, per [rust-patterns.md](../rust-patterns.md) §7 — not `init_tracing`)
- Build `SharedBus` with the TCP bridge to the Pi
- Start `fouche` pipelines (camera → perception → publish)
- Start `fouche::intent` (subscribe to command source, call `IntentDecomposer`, publish `NavigatorIntent`)
- Publish `HostMetrics` with `JetsonPlatformMetrics` to `host/metrics/jetson`
- Publish `Heartbeat` on `heartbeat/jetson`
- Optionally subscribe to `robot/state` for perception context

### 11. Pi side — subscribe to Jetson topics

Talleyrand subscribes to `navigator/intent`, resolves `target_label` → `CartesianPose` via the labeled-pose catalog, runs IK per phase, emits joint trajectories to Berthier. **Berthier and Davout do not change.** That is the architecture paying off — the joint-space executor and safety gateway are independent of where intent came from.

### 12. Deploy + systemd + tooling

- Cross-build for aarch64 (existing cross-build smoke test per AGENTS.md).
- Add `scripts/jetson-remote.sh` paralleling `pi-remote.sh` (verify, health, logs, deploy).
- Add `marengo-jetson.service` systemd unit on the Jetson, ordered after `network-online.target`.
- Add a `jetson-sync-main` MCP step paralleling `pi-sync-main`.

### 13. Gateway / Consul

Gateway already bridges Chappe to Consul. Jetson messages reach the gateway via the TCP bridge → Pi → gateway. **No new gateway on the Jetson.** Consul gains Jetson host card (already proto-supported) and perception/intent surfaces (new topics, allowlist them in ADR 0008's WebTransport subscribe set when ready).

## Consequences

- New `chappe::transport::net` module + `TcpBridge` (length-prefixed `Envelope` framing).
- New proto messages: `CartesianPose`, `MotionPrimitive`, `MotionPhase`, `NavigatorIntent`, `DetectedObject`, `PerceptionFrame`.
- `crates/fouche` fleshed out: `camera`, `perception`, `intent` (with `IntentDecomposer` trait + OpenRouter/local impls), `primitives` catalog.
- `bins/marengo-jetson` fleshed out (Chappe producer, host metrics, heartbeat, pipeline orchestration).
- `crates/talleyrand` gains `NavigatorIntent` subscription + labeled-pose resolution + per-phase IK.
- Berthier, Davout, robstride: **unchanged.**
- New deploy path: aarch64 cross-build + `jetson-remote.sh` + `marengo-jetson.service`.
- LLM cost/latency budget: OpenRouter default (~$0.01–0.05 per decomposition, 2–3 s); local 3070 Ti opt-in.

## Alternatives considered

- **LLM emits raw Cartesian waypoints.** Rejected — LLMs are unreliable at exact 3D coordinates, no notion of timing/smoothness, unreachable poses. Structured motion spec + classical IK is the proven pattern.
- **Run the LLM on the Jetson.** Rejected for the 8 GB variant — vision + 7B LLM OOMs or thrashes. Possible later with a 16 GB Orin NX or by pausing vision during inference; not the default.
- **Zenoh / NATS for Pi↔Jetson.** Deferred — two nodes on a dedicated subnet don't need discovery/multicast. TCP bridge is swappable later.
- **Reuse the gateway WebTransport for Jetson↔Pi.** Rejected — couples Jetson to gateway uptime; Jetson↔Pi must work headless.
- **LLM directly emits joint targets.** Rejected — bypasses Talleyrand IK and the URDF joint-space safety envelope (ADR 0009, ADR 0010). The joint-space executor and Davout safety gateway must stay the sole path to motors.
- **Move IMU to Jetson.** Rejected — control loop needs it at tick rate; I2C on Pi keeps latency bounded.

## Implementation order (when the shelf is done)

1. **Physical wiring + Ethernet subnet** — verify `ping` before any code.
2. **Chappe TCP bridge** — implement `chappe::transport::net`, test with `Heartbeat` round-trip Pi↔Jetson.
3. **Proto additions** — `CartesianPose`, `MotionPhase`, `NavigatorIntent`, `PerceptionFrame`; regenerate.
4. **`marengo-jetson` bin fleshing** — host metrics + heartbeat first (proves the bridge), then `fouche` pipelines.
5. **`fouche::intent` + prompt design** — OpenRouter impl first; refine primitive catalog until "wave" / "point at ceiling" / "reach forward" decompositions are reliable.
6. **`fouche::perception`** — YOLOv8n INT8 in TensorRT; publish `PerceptionFrame` at 5–10 Hz.
7. **Talleyrand consumes `NavigatorIntent`** — labeled-pose catalog + per-phase IK → Berthier. Closes the loop.

## Work that can happen now, before the shelf

- **Prototype the prompt against OpenRouter** — give Claude/GPT the primitive catalog as tools, try "wave" / "point at ceiling" / "reach forward slowly". Refine until decompositions are reliable. Zero hardware needed.
- **Design the motion primitive catalog** — list the 10–20 primitives that cover reachable behaviors. Notebook work.
- **Design the labeled-pose catalog** — tie names to the URDF `base_link`.
- **Run `llama-bench` on the 3070 Ti** — measure actual tok/s for 7B Q4_K_M to validate local-LLM viability.
- **Pick a YOLO variant for the Jetson** — YOLOv8n INT8 in TensorRT is the standard Orin Nano starting point.

The shelf is the long pole; the prompt + primitive catalog is the thing that determines whether "wave" actually works, and it can be nailed this week with just an OpenRouter key.

## References

- [architecture.md](../architecture.md) — Pi / Jetson / Fouché / Chappe data flow
- [rust-patterns.md](../rust-patterns.md) §3, §7 — crate boundaries, control law, coordinate ownership
- [ADR 0001](0001-protobuf-wire-types.md) — protobuf wire types
- [ADR 0008](0008-chappe-webtransport-transport.md) — Chappe gateway transport (length-prefix framing reused)
- [ADR 0009](0009-dynamic-position-limit-envelope.md) — joint-space safety envelope (preserved by this design)
- [ADR 0010](0010-actuator-velocity-cap-resolution.md) — velocity cap (preserved by this design)
