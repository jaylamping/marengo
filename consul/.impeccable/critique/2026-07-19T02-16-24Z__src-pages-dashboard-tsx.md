---
target: dashboard
total_score: 17
p0_count: 2
p1_count: 2
timestamp: 2026-07-19T02-16-24Z
slug: src-pages-dashboard-tsx
---
# Critique: Consul Dashboard Overview

**Target:** `src/pages/dashboard.tsx` → `DashboardOverview`  
**Method:** dual-agent (A: 33193277-008b-47bf-b6e9-2e3146ce9f81 · B: d31eec01-b172-433f-960f-6a671725bf82)  
**Browser overlays:** unavailable (IDE browser tabs evaporated; no detect.js inject)  
**CLI detector:** exit 0, 0 findings on scoped dashboard paths

## Design Health Score

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 2 | Header WIREFRAME LED is honest; cards still read healthy/live on dummy data |
| 2 | Match System / Real World | 2 | Domain language fits; Account/Logout/Share do not |
| 3 | User Control and Freedom | 2 | Dead `#` nav and non-functional menus offer fake exits |
| 4 | Consistency and Standards | 2 | Soft radii / inset stripe drift from Launch Day 4px instrument system |
| 5 | Error Prevention | 2 | Fake calm status invites wrong trust under wireframe |
| 6 | Recognition Rather Than Recall | 2 | Live routes indistinguishable from stubs |
| 7 | Flexibility and Efficiency | 1 | No shortcuts; Search is `#`; no command palette |
| 8 | Aesthetic and Minimalist Design | 1 | Placeholder card + kit menus violate no-empty-structure |
| 9 | Error Recovery | 2 | Overview has no diagnose→fix path for faults |
| 10 | Help and Documentation | 1 | Docs/Search dead; no contextual help on badges |
| **Total** | | **17/40** | **Poor** |

## Anti-Patterns Verdict

**LLM assessment:** Fails the product slop test. Launch Day tokens (IBM Plex, blue-black, amber phosphor) are distinctive, but the composition is a stock shadcn KPI dashboard wearing that paint: identical 4-up cards, TBD placeholder that exists to keep the grid, SaaS user menu (Account/Logout), presets Share/Delete, active-nav inset accent stripe (absolute ban), hero-metric power title.

**Deterministic scan:** `detect.mjs --json` on overview/section-cards/layout/chart/header/sidebar → **0 findings**. Detector missed the structural kit tells (it does not flag purposeless placeholders or fake-live semantics). Treat zero findings as a blind spot, not a clean bill.

**Visual overlays:** No reliable user-visible overlay — browser mutation/injection blocked.

## Overall Impression

The skin is ready for an operator console; the Overview layout is still answering “are the hosts happy?” instead of “is the arm where I think it is, and is Davout calm?” Biggest opportunity: kill empty structure and make wireframe mode visually honest end-to-end, then put posture + tracking above the fold as one continuous thought.

## What's Working

1. **Launch Day tokens** — restrained chroma, IBM Plex Sans/Mono, semantic LEDs, amber-as-armed.
2. **Header machine-state chip** — scarce chroma, uppercase mono; the right at-a-glance pattern.
3. **Joint tracking limits/safety bands** — real instrumentation, not decorative chart chrome.

## Priority Issues

### [P0] Status honesty collapse — dummy presented as live
- **What:** Chart titles always `(live)` even on `dummyShoulderPitchTracking`. Power/host cards default to dummy “healthy / discharging.”
- **Why:** False calm at 2am is worse than empty. Breaks Precise · Calm · Armed.
- **Fix:** Wireframe must look wireframe — DEMO labels, muted badges, never `(live)` on dummy.
- **Suggested command:** `/impeccable clarify` (then harden)

### [P0] Empty structure — placeholder card + kit leftovers
- **What:** `OverviewPlaceholderCard` (“Keeps the 4-column overview grid”); `SidebarUserMenu` Account/Logout; presets Share/Delete on `#`.
- **Why:** Violates PRODUCT “no empty structure” and anti-ref stock dashboard.
- **Fix:** Delete fourth card (or replace with real Davout/CAN signal). Gut user menu to bench identity or remove. Hide presets until real.
- **Suggested command:** `/impeccable distill`

### [P1] No posture / presence on Overview
- **What:** Dust backdrop only; no URDF posture; Visualizer nav is `#`. Chart is delayed and secondary.
- **Why:** PRODUCT success: health *and posture* in seconds unmet.
- **Fix:** Compact pose co-located with tracking; demote host KPIs to a dense strip.
- **Suggested command:** `/impeccable layout`

### [P1] Dead-nav wall as equal peers
- **What:** Visualizer, Safety, Telemetry, Settings, Docs, Search → `#`, same weight as live routes.
- **Why:** Recognition fails; burns trust and clicks.
- **Fix:** Ship only live routes; park stubs or remove.
- **Suggested command:** `/impeccable distill`

### [P2] Composition / ban drift
- **What:** Equal KPI grid; inset accent stripe on active nav; rounded-xl / rounded-full vs 4px instrument radius.
- **Why:** Reads as soft shadcn, not Launch Day console.
- **Fix:** Unequal hierarchy; replace inset stripe; harden radius.
- **Suggested command:** `/impeccable quieter` / `/impeccable layout`

## Persona Red Flags

**Alex (power user):** Search is `#` (expected ⌘K); no keyboard accelerators on time range; stub peers force exploration-by-click; equal KPIs slow “what’s wrong?” scan.

**Joey (sole operator, 2am bench):** Needs fault/mode/pose; gets CPU/RAM + TBD. Fake healthy while header says WIREFRAME. Subtitle `marengo_arm_4dof` vs shoulder-pitch-only URDF. “Log out” is nonsense on a local cockpit. No path Overview → Testing/Logs as one continuous thought.

## Cognitive Load

**6/8 checklist failures → high.** Equal-weight KPI row, no single focus, stub wall, chart joint + time-range decision overload.

## Minor Observations

- `UsageBar` rounded-full amber on nominal CPU reads decorative.
- Chart description claims Chappe measured even on dummy fallback.
- SidebarBrand links to `#`.
- DeferredMount 800ms delays the only somewhat-useful instrumentation.

## Questions to Consider

1. If Overview answers one question in three seconds, is it host health or arm posture + Davout calm?
2. Would deleting the placeholder card and user-menu dropdown make Consul feel more finished?
3. Should WIREFRAME mute the entire page, or stay a header badge while cards role-play production?
