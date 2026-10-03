# Crate audit 2026-10-03

Audit of every workspace crate and bin (Davout first): what each one is for, where it has gaps and bugs, and what can be pruned. This directory holds the Phase A output (intent cards and metrics) and its synthesis. Phase B (the bug hunt) works from `leads-index.md`. The prune list needs user approval before anything is deleted.

## Baseline

- **Code baseline:** `a2b55b3` ("fix(davout): hold each Enable past its drive's post-SetZero blackout"), worktree `/Users/joseph/code/marengo-wt/audit`, branch `audit/2026-10-03`.
- **Commits after the baseline that change findings:** `c879b6a` (Berthier/marengo-pi enable-completion gate), `c11d400` (CAN-opening homing preflight removed), `9bf46d8` (`pi_enable_soak` MCP tool). Unmerged: `fix/rs03-velocity-scale` (RS03 velocity range ±20 rad/s). Their effect is recorded in `leads-index.md` and `prune-candidates.md`.
- **Toolchain for metrics:** rustc 1.88.0, cargo-llvm-cov 0.9.1, cargo-machete 0.9.2, cargo-deny 0.20.2, cargo-audit 0.22.2 on aarch64-apple-darwin (`metrics/README.md`). Linux-only code (SocketCAN backend, host-metrics collector, I2C backend, journal import) is not in the coverage numbers.

## Method

1. **Metrics** (`metrics/`): workspace coverage (81.7 % lines), LOC, test counts, suppressions, unused deps, cargo deny/audit (both pass), a public-symbol usage heuristic.
2. **Intent cards** (`intent/<crate>.md`, 28 cards): one per crate/bin, same 10 sections — header, intent (with conflicting statements), owns/must-not, interface and consumers, invariants with enforcing code and tests, inputs/outputs, reconciliation with the 2026-09-29 review, doc drift, prune candidates, Phase B leads. Each claim cites a file:line.
3. **Synthesis** (`README.md`, `intent-map.md`, `prune-candidates.md`, `leads-index.md`, `prior-review-status.md`): deduplicated prune candidates and leads, the crate map, and the prior-review status. Every row cites its source card section. Where two cards disagreed, the code was read to settle it (noted in the file). No finding was invented. The integrator's own observations (`local://audit-leads.md`) were folded in.
4. **Cross-checks:** `safety-invariants.md` (safety-invariant matrix: 210 documented rules and 283 code invariants traced to code and tests) and `contracts.md` (config, proto, route and topic inventory) were written in parallel and folded in once they appeared (matrix read as of its 12:01 revision). Their leads and prune-class flags are merged into the synthesis files with their row/flag ids; items only they found are marked as such.

## How to read it

- Start with **`prune-candidates.md`**: the DECIDE table at the top lists the user decisions; everything else is grouped into commit-sized batches.
- Then **`leads-index.md`**: leads are ranked by severity (S1 = can move a motor wrongly or defeat a stop/fault … S4 = hygiene) and likelihood on the current bench, and grouped into Phase B work packages (WP-A … WP-T).
- **`intent-map.md`** explains what each crate is supposed to own, the runtime/tooling diagram, every boundary violation and doubly-owned concept, and doc drift.
- **`prior-review-status.md`** says which 2026-09-29 findings are fixed, open or superseded, and where the finding index or ledger is out of date.
- For detail on any row, open the cited `intent/<crate>.md §n`.

## Headline numbers

- **Prune candidates:** 183 — PRUNE 79, PRUNE-AFTER 61, DECIDE 23, KEEP 20; 11 user decisions (D-1 … D-11).
- **Leads:** 309 — 3 resolved after a2b55b3; open S1 31, S2 79, S3 128, S4 68. One S1 is confirmed on the bench (RS03 velocity scale, fix in progress on `fix/rs03-velocity-scale`).
- **Prior findings tracked:** 71 ids; 8 cases of finding-index drift and 6 of ledger drift.

## File index

| File | Contents |
|---|---|
| `README.md` | This file |
| `intent-map.md` | Per-crate intent/owns/must-not/consumers/depth; mermaid diagram; boundary violations (V1–V14); doubly-owned concepts (D1–D31); doc-drift summary |
| `prune-candidates.md` | DECIDE table; commit batches B1–B16; all prune candidates with evidence, confidence, blast radius and dependencies |
| `leads-index.md` | Severity rubric; post-baseline status changes; ranked leads with file:line, reproduction idea, prior ids and merged sources; work packages |
| `prior-review-status.md` | Every 2026-09-29 finding id referenced, with ledger/index/current status and drift |
| `intent/*.md` | 28 Phase A intent cards (armee-dynamics, armee-kinematics, armee-proto, berthier, chappe, davout, fouche, imu-probe, marengo-candump, marengo-config, marengo-deploy, marengo-gateway, marengo-homing, marengo-host-metrics, marengo-imu, marengo-jetson, marengo-limit-sync, marengo-log-cli, marengo-pi, marengo-store, marengo-support, motor-repl, probe, robstride, sim-harness, talleyrand, teleop, wave-demo) |
| `metrics/*.md` | Coverage by crate/file, LOC, test counts, suppressions, pub usage, unused deps, deny/audit logs |
| `safety-invariants.md` | Safety-invariant traceability matrix (parallel Phase A output) |
| `contracts.md` | Config/proto/route/topic contract inventory (parallel Phase A output) |
