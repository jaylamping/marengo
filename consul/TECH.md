# Consul 2026 Technology Surface

This document captures the **current best-in-class** choices for building the sexiest, smoothest, most functional operator cockpit possible in mid-2026.

It is deliberately opinionated and will be updated as the real sharp edges move.

## Core Application

| Concern              | Choice                          | Rationale |
|----------------------|----------------------------------|---------|
| Framework            | **Vite 6 + React 19 + TypeScript** | React 19 compiler is the single biggest smoothness win available. Vite is still the fastest DX. |
| Language             | TypeScript (strictest config)   | Non-negotiable for a codebase you will live in daily. |
| Styling              | **Tailwind CSS 4** + shadcn/ui (Radix primitives) | Best density + customization balance. shadcn gives us beautiful, accessible building blocks without bloat. |
| Resizable Layout     | `react-resizable-panels` + shadcn `Resizable` wrapper | Exactly the Foxglove-style draggable panes we need. |
| Command Palette      | `cmdk` (or kbar if we need more) | ⌘K to reach *anything* is table stakes for a functional tool. |

## 3D Visualizer (First-Class Surface)

| Concern                  | Choice                                      | Notes |
|--------------------------|---------------------------------------------|-------|
| 3D Engine                | **@react-three/fiber** + **@react-three/drei** + `three` (latest) | The modern React way. Fiber gives us declarative React components over Three. |
| URDF / Robot Loading     | Custom loader on top of `three` + `urdf-loader` or our own parser | We will own the loading + joint-driven scene graph so we can do exactly what we want (ghosting, exploded views, contact viz, etc.). |
| Performance & Polish     | `three` r128+, instanced meshes, proper frustum culling, LOD for complex meshes | Smooth 120 Hz+ camera + selection is non-negotiable. |
| Interaction              | Pointer events + raycasting + custom orbit controls | Selection must feel instant and precise. |

**Inspiration baseline**: URDF-Studio + urdf-viewer (2026 versions) — we will start from their loading/interaction quality and then make it *ours*.

## Data, Time, & State

| Concern                    | Choice                              | Why |
|----------------------------|-------------------------------------|-----|
| Global Robot State         | **Zustand** (with Immer middleware) | Tiny, excellent TS, great for time-travel / snapshotting later. |
| Time-synced Surfaces       | Single source-of-truth time cursor + derived stores | Every plot, the 3D model, the event list, and the joint inspector must be perfectly locked. |
| Live Plots / Telemetry     | Recharts (or uPlot / @visx for higher perf) | Recharts is good enough for 90% of use cases and has great React 19 ergonomics. We can swap the hot path later. |
| Session Recording / Replay | Event log + protobuf snapshots + time cursor | Built from day one so scrubbing is perfect. |

## Messaging & Transport

- **Bench / dev**: [`marengo-gateway`](../bins/marengo-gateway/) — HTTP CRUD on `:8080`, WebTransport on `:8443` ([ADR 0008](../docs/decisions/0008-chappe-webtransport-transport.md)).
- **Wire**: Binary protobuf (`@bufbuild/protobuf`, `consul/src/gen/marengo/v1/marengo_pb.ts` after `npm run gen:proto`).
- **Env**: `VITE_CHAPPE_HTTP_URL`, `VITE_CHAPPE_WEBTRANSPORT_URL` (see `consul/.env.example`). Pi access via `ssh -L`.
- **No GraphQL, no WebSocket fallback** in phase 1.
- **Client**: `src/lib/chappe-client.ts`, `useChappeTelemetry` in app providers.

## Other Sharp Tools (2026)

- **React 19 features**: `use`, Actions, the compiler — we will lean on them heavily for perceived performance.
- **State & URL sync**: Keep important workspaces shareable via URL (lightweight).
- **Theming**: Single dark theme only (high-contrast, lab-friendly). No light mode.
- **Dev UX**: Lightning-fast HMR, excellent source maps, React DevTools Profiler as a first-class citizen.
- **Linting / Formatting**: Same strict rules as the Rust side (we will mirror the spirit of the workspace lints).

## Explicit Non-Choices (for now)

- No heavy 3D frameworks (R3F is enough).
- No “dashboard widget” libraries (we will compose from primitives).
- No Electron (browser + WebTransport is the future we want).
- No legacy ROS webviz / rosbridge patterns unless they are dramatically better (they are not).

---

## How to Use This Document

When evaluating a new library or pattern:

1. Does it make the 3D visualizer or the information surfaces *noticeably* smoother or more powerful?
2. Does it increase or decrease the friction of turning a software knob?
3. Does it respect the “information is sacred” rule?

If it fails any of the above, we don’t take it.

This stack is chosen so that in 2026–2027 Consul can be the best-feeling robotics operator tool that exists — not the prettiest marketing screenshot, but the one you never want to leave when you’re actually working on the robot.

---

**Current status (May 2026)**: Dashboard shell + live Chappe telemetry path (gateway + WebTransport client). URDF-first 3D and session replay are next.
