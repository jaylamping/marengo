# Intent card: fouche

## 1. Header

| Field | Value |
|---|---|
| Crate | `fouche` |
| Path | `crates/fouche` |
| Kind | lib (scaffold; no features) |
| Baseline | `a2b55b3` |
| LOC src/tests | 1 / 0 (`src/lib.rs:1` is a single `//!` line; metrics/loc.md:15). 0 tests |
| Sources | `src/lib.rs:1`; `Cargo.toml`; `README.md`; `codemap.md`; `src/codemap.md`; AGENTS.md:21; crates/AGENTS.md:16; docs/architecture.md:27; docs/rust-patterns.md:28; docs/roadmap.md:28,167; ADR 0014 (whole); ADR 0001 Rationale ("future Python"); `models/README.md`; `consul/src/data/logs.ts:22-29`; repository-review.md:89; metrics/unused-deps.md; `git log` (4 commits, docs/housekeeping only: `61fe36d`, `5106357`, `dd2ed3e`, `5b4f222`). |

## 2. Intent

Fouché is the reserved home of **Jetson-side perception and semantic intent**: camera pipelines, on-device vision inference, and a client that decomposes natural-language commands into a structured motion spec. It publishes on Chappe for Talleyrand and Consul and never touches control (lib.rs:1; README.md:7-9; crates/AGENTS.md:16; architecture.md:27). ADR 0014 (Accepted 2026-06-19) is the authoritative design:
- Module plan: `camera`, `perception` (YOLOv8n INT8/TensorRT, publishing `PerceptionFrame` on `perception/frame` at 5-10 Hz) and `intent`. The intent module has an `IntentDecomposer` trait with `OpenRouterClient` and `LocalLlamaCpp` impls and publishes `NavigatorIntent` on `navigator/intent`. A primitives/labeled-pose catalog may live here or in Talleyrand (ADR 0014 §4-§10, Consequences).
- Hard rule: **Jetson is perception + intent, never control**. No CAN, motors, `davout`, `robstride` or real-time loops (ADR 0014 §1).
- The LLM emits primitives from a fixed catalog, never raw Cartesian waypoints or joint targets (ADR 0014 §4, Alternatives).
- The roadmap gate is "After Jetson role and ONNX scope defined" (roadmap.md:167). ADR 0014 defines the role. [INFERENCE] The "ONNX scope" half is superseded by ADR 0014's TensorRT choice but was never reconciled.

Conflicting statements of intent:

| Topic | Source A | Source B |
|---|---|---|
| Where the LLM runs | ADR 0014 §8 "Jetson does vision only. LLM lives off the robot"; fouche only hosts the *client* | README.md:9 "optional LLM tooling" on the Jetson; crates/AGENTS.md:16 and rust-patterns.md:28 "Vision / LLM (Jetson-side)" |
| Model format | ADR 0014 §8, Implementation 6: YOLOv8n INT8 in TensorRT | README.md:9 and models/README.md:3 "ONNX policies"; roadmap.md:167 "ONNX scope"; codemap.md:7 "ONNX policy loading" |
| "Policies" | README/models say *policies* (implies learned control) | ADR 0014 §1 forbids control on the Jetson. [INFERENCE] "policies" is a pre-ADR term |

## 3. Owns / Must not

| Owns (future) | Evidence |
|---|---|
| Camera → perception → `PerceptionFrame` | ADR 0014 §7, §10 |
| `IntentDecomposer` (LLM client) → `NavigatorIntent` | ADR 0014 §8 |
| Possibly the primitive / labeled-pose catalog | ADR 0014 §5 ("TBD") |

| Must not | Evidence | Status |
|---|---|---|
| CAN, motors, davout, robstride, RT loops | ADR 0014 §1 | No code, so trivially satisfied |
| Emit joint targets or raw Cartesian waypoints | ADR 0014 Alternatives | n/a |
| Host LLM inference on the 8 GB Jetson by default | ADR 0014 §8 | n/a |

## 4. Interface

No public items (lib.rs:1). Declared deps `thiserror`, `tracing` (Cargo.toml:13-15) are unused per `cargo machete` (metrics/unused-deps.md). The only dependent, `marengo-jetson` (Cargo.toml:19), is also flagged as not using it (metrics/unused-deps.md); its `main.rs:3-6` only logs "scaffold". The one non-Rust reference is `'fouche'` in Consul's **wireframe** log sample data (`consul/src/data/logs.ts:1,22-29`: "Log wireframe data — replace with Chappe / tracing stream later"), which is not a live contract. Depth: none. Seams planned: `IntentDecomposer`, one trait with two planned adapters (OpenRouter, local llama.cpp), so it would be a real seam (ADR 0014 §8).

## 5. Invariants owned

None in code. The intended invariant (no path from the Jetson/LLM to motors except Chappe → Talleyrand → Berthier → Davout) is upheld today by absence. Untested.

## 6. Inputs / outputs

None today. Planned (ADR 0014 §2, §7, §8): MIPI CSI cameras; HTTP to OpenRouter or a local `llama-server` (config-toggled, default OpenRouter); topics out `perception/frame`, `navigator/intent`; optional in `robot/state`. Model weights in `models/` (Git LFS; currently `.gitkeep` + README only). Needed proto messages `PerceptionFrame`, `DetectedObject`, `NavigatorIntent`, `MotionPhase`, `CartesianPose` do not exist yet (grep of marengo.proto: none). Jetson host telemetry (`JetsonPlatformMetrics`, `HOST_NODE_ROLE_JETSON`, marengo.proto:214,245,393) already exists. That is marengo-jetson's job, not fouche's (ADR 0014 §7, §10).

## 7. Prior review reconciliation

| Prior id | Topic | Status | Evidence |
|---|---|---|---|
| repository-review.md:89 | Perception scaffold, no working vision stack | **unchanged (accurate)** | lib.rs:1 |
| implementation-roadmap.md:117 | Don't grow scaffolds for unrelated findings | **respected** | no code commits |
| ledger M07 | Scope honesty for scaffolds | **partial** | AGENTS.md:21 "scaffold"; README.md:9 and models/README.md describe capabilities as present-tense |

## 8. Drift

- README.md:9 "Camera pipelines, ONNX policies … optional LLM tooling" is written in the present tense; nothing exists, and it contradicts ADR 0014 §8 (LLM off-robot; TensorRT).
- codemap.md:7 / src/codemap.md:4 "Vision pipeline stubs and Jetson integration hooks": there are no stubs or hooks, only one doc line.
- roadmap.md:167 gate wording ("ONNX scope") is not updated for ADR 0014.
- ADR 0014 Context says "1-line `crates/fouche`", which still holds. Its "Work that can happen now" (prompt and primitive catalog design) has no artifact in the repo [INFERENCE from grep for `MotionPrimitive`/`IntentDecomposer`: none outside ADR 0014].

## 9. Prune candidates (classification only; deleting roadmap scaffolds is a user decision)

| Candidate | Evidence class | Confidence | Keep cost | Delete cost / touches |
|---|---|---|---|---|
| Crate `fouche` | Scaffold with no consumer (0 pub items; the only dependent does not use it) | n/a (user decision) | One workspace member; two unused deps (metrics/unused-deps.md); README/model docs that contradict ADR 0014 | Root Cargo.toml:15,67; bins/marengo-jetson/Cargo.toml:19 and codemap.md:11; README.md:65; AGENTS.md:21; crates/AGENTS.md:16; crates/codemap.md:25; codemap.md:33; docs/architecture.md:27; docs/rust-patterns.md:28; docs/roadmap.md:167; ADR 0014 (would need a superseding note: its Consequences name `crates/fouche` modules); docs/portraits/fouche.jpg + README row; models/README.md:3; consul/src/data/logs.ts:28 (wireframe sample only). Recreating later costs one Cargo.toml + lib.rs |
| Unused deps `thiserror`, `tracing` | Scaffold with no consumer (machete) | high | — | fouche/Cargo.toml:13-15 |
| `'fouche'` in Consul `LOG_SOURCES` | Scaffold sample data (logs.ts:1 "wireframe") | low (Consul owner's call; affects mock log rendering only) | — | consul/src/data/logs.ts:28 |

## 10. Phase-B leads

| # | Location | Why suspicious |
|---|---|---|
| F1 | ADR 0014 §3 / §13 | Fouché's whole output path depends on a Chappe TCP bridge (`chappe::transport::net`) and gateway allowlisting that do not exist. A Jetson publisher would also need Envelope admission/authorization on the Pi side. No ADR covers authenticating `navigator/intent` from the network (ADR 0033 covers gateway runtime access only) [INFERENCE] |
| F2 | ADR 0014 §8 | An off-robot LLM (OpenRouter) in the command path means network latency/outage and prompt-injection risk feed motion intent. The ADR's mitigation is the fixed primitive catalog plus Talleyrand IK plus the Davout envelope, and none of it exists yet. Flag for safety review before any implementation |
| F3 | README.md:9; models/README.md:3 | "ONNX policies" wording could invite a learned-control path on the Jetson, contrary to ADR 0014 §1 |
