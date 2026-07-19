# Consul Design

**Consul** is the personal daily-driver operator cockpit for Marengo.

It exists for one person (you) who will live in it for hours every day while building, debugging, and tuning a 23-DOF humanoid. It must be the sexiest, smoothest, most *functional* robotics operator interface on the planet in 2026.

## The Only Filter

Every pixel, interaction, and feature must survive this question:

> Does this make me faster, more certain, or more in control of the physical machine right now?

If the answer is “no” or “only a little,” it does not exist.

## Current Phase

Consul is **wireframe + live telemetry** when `VITE_CHAPPE_*` is set. KPI cards, charts, inventory rows, and header badges still use placeholders by default; joint tracking, safety/mode badges, and inventory values can reflect real `RobotState` / `SafetyState` from `marengo-gateway` over WebTransport.

The **Simulation** tab is the planned home for Isaac Sim / Isaac Lab: viewport stream, play/pause/scenario controls, runtime metrics, and task logs. Same wireframe rules apply until the D2 Lab bridge shares `proto/` with the stack ([ADR 0003](../docs/decisions/0003-simulation-testing.md)).

## Core Principles

### 1. Information is sacred
- The goal is **understanding**, not data volume.
- Raw logs, metrics, and messages are only valuable when transformed into **actionable, time-synced, context-rich surfaces**.
- No infinite listviews. No firehoses. No “everything everywhere all at once.”
- The 3D view, plots, joint inspector, safety state, and event stream are all locked to the same time cursor. Scrubbing tells a story.

### 2. The 3D visualizer is first-class
- This is not a widget. It is the primary way the physical robot feels *present* on screen.
- Selection, highlighting, measurement, camera choreography, ghosting, contact viz, and exploded views must feel native and delightful.
- Tight bidirectional coupling: click a link in 3D → the rest of the app snaps to that joint/state. Scrub time → the model moves with every other surface.

### 3. Every tunable thing is a knob or dial
- If the robot exposes it in config, YAML, or at runtime (gains, limits, gravity compensation, safety zones, friction models, presets, behaviors…), it must be reachable and changeable from Consul with the same directness as turning a physical potentiometer.
- Changes must be instant to see in the 3D view and live plots.
- Perfect recall, versioning, and one-click restore of any previous “I liked it when it felt like this” state.

### 4. Smoothness and immediacy are non-negotiable
- The UI must feel *obscenely* good. 60 fps interactions, zero perceptible lag on knob drags or 3D camera moves, command palette that appears before you finish thinking the shortcut.
- React 19 compiler, modern web primitives, and obsessive attention to frame times and input latency.

### 5. Modern stack only
- We use the sharpest tools available in 2026 (React 19, WebTransport for messaging when the transport ADR lands, whatever state + data solution actually feels lightest and most powerful).
- We do not inherit 2018–2024 ROS/webviz patterns because “that’s what everyone does.”

## What “Functional + Sexy” Actually Looks Like Here

- You open Consul and in < 3 seconds you know the health and posture of the entire machine.
- You want to change a single joint’s `kd` while watching the 3D arm and the torque error plot — it feels like one continuous thought.
- When something goes wrong at 2 a.m., the thing that matters (the violated limit, the bad gain, the exact pose) is already highlighted or suggested.
- The interface disappears when you are in flow and only appears when you need leverage.

## Nice-to-Haves (for later)

- Teleoperation (high-quality, low-latency, with the same 3D + knob surface)
- Session recording + perfect replay with parameter snapshots
- Multi-robot / fleet views (only after the single-robot experience is god-tier)

## Anti-Patterns (will be rejected on sight)

- Marketing chrome, big logos, “delightful” animations that cost attention
- Enterprise SaaS visual language (Formant-style)
- Generic “dashboard” widgets that look good in screenshots but slow you down in practice
- Anything that makes the 3D view feel secondary
- Any feature whose primary justification is “other people might find this useful”
- GLINUI signature components (Meteor Shower, Typewriter, Pulsating Button) on operator data routes
- Frosted glass / backdrop blur on any surface (retired 2026-07 — the “Launch Day” reskin)
- Decorative motion or idle “alive” effects other than the dust backdrop drift
- Gradients for beauty (legibility aids like the vignette excepted)
- Playful empty states, illustrations, easter eggs

## Visual identity — “Launch Day”

Consul looks like the console a SpaceX engineer has open on launch day: calm, dense, perfectly aligned mono telemetry on near-black, where color is so scarce that a hue change means something. This is instrumentation for expensive hardware — nothing playful survives review.

- **Surfaces:** opaque layered panels (`--surface-0..3`) on a blue-black void, hairline borders (`--line` / `--line-strong`). No blur, no frost.
- **Accent:** amber phosphor (`--accent`, ≈ #FFB000) with armed/caution semantics — active controls, armed machine state, chart-commanded series, focus rings. Never decoration.
- **Status palette:** `--ok` green, `--warning` amber, `--fault` red, `--info` cyan (sparing). Nothing else carries chroma.
- **Type:** IBM Plex Sans for UI text; IBM Plex Mono for every number, log line, and micro-label (10px uppercase, 0.14em tracking). No display fonts.
- **Radius:** 4px. Sharp instruments, not consumer cards.
- **LEDs (`.led*`):** the only always-on glow; live-link pulses at 2.4s (reduced-motion aware).
- **Corner brackets (`.panel-brackets`):** semantic only — surfaces that act on hardware (e-stop, enable, armed tuning panels). Never on passive cards.
- **Dust backdrop:** soft gray particles drift slowly in a near-black void behind the fullscreen canvas. The air moves; the robot (telemetry) stays rock solid. This is the single sanctioned ambient motion — procedural, no stock imagery.

### Surface and motion tiers

| Tier | Surfaces | Skin | Motion |
|------|----------|------|--------|
| Chrome | sidebar, header, nav, tabs, control bar | opaque panel + hairline | mount ≤200ms; hover feedback |
| Data | tables, charts, log/inventory rows | panel shell; opaque rows | sort/filter ≤150ms; **no row animation** |
| Hero | empty states, sim idle, welcome | panel + void | stagger enter/exit allowed |

Glow is semantic only: armed (amber), fault (red), live-link (LED). Nominal state never glows. `prefers-reduced-motion` freezes the dust drift and LED pulse. Telemetry springs stay unchanged.

---

This document is the living filter. When in doubt, come back here. If a proposed component, layout, or library cannot be defended against these principles, it does not ship.

The bar is extremely high. That is the point.
