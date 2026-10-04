---
name: control-rewrite
description: How to change Marengo's control logic - Berthier's position law, hold, trajectory, feed-forward (gravity, friction, damping, inertia), gain runtime and degraded lower, armee-dynamics models and fits, and the computation inside Davout's command filter. Rewrite boldly when derivation and evidence show the current code is wrong or inferior, make the full case for the change, and verify it in tests and on the bench. Use whenever you are about to modify, refactor or replace control code, or when an audit, trace or bench diagnosis concludes the control logic itself is at fault.
---

# Control rewrite

The owner wrote much of the low-level control code early and does not trust it. Treat it as a draft to improve, not a design to preserve. When you are convinced that a piece of control logic is wrong, or worse than an alternative you can derive, replace it: the whole function, module or law if that is what the better design needs. A minimal patch that keeps a flawed structure alive is the wrong outcome here, and nobody is attached to the old code.

Conviction has to rest on a derivation and on evidence, and the explanation is part of the deliverable. The bar for a large change is a stronger case, not a smaller diff.

## Open to rewrite

- **Berthier:** the position law, hold, setpoint, trajectory, profile and wave; the feed-forward composition (how τ_g is used, friction, damping, J·a); gain runtime and ramps; the degraded lower.
- **armee-dynamics:** gravity models, lumped models and calibration fits.
- **Davout:** how the command filter shapes a command (rate limits, the total-torque prediction, envelope arithmetic), as long as it keeps the guarantee it provides.
- **Code organisation:** names, module boundaries inside a crate, and dead code (delete it).
- **Tests:** rewrite tests that pinned incidental details of the old behaviour so they assert the new contract.

## Fixed, whatever the rewrite

These are the robot's safety contract (`AGENTS.md`, `docs/safety.md`), and they are what keeps a bold rewrite safe to try:

- The motor path Berthier → Davout → robstride, and each crate's ownership (Davout owns joint↔motor, Berthier never opens CAN).
- Every safety mechanism keeps its guarantee: fuses, latches, reference grants, caps, pacing, watchdogs, the limit envelope, the total-torque clamp. You may reimplement one if it gives the same guarantee. Changing *what* it guarantees needs an ADR.
- Physical numbers (gains, velocities, caps, limits) change only with bench evidence. A new law usually needs new numbers: derive them, then confirm them on the bench at reduced speed.
- An elevated arm holds in GravityComp, and a hold trip at home means a model fault.

## The case for a sweeping change

Write the case before the code. Put it in the commit body, or for a large change in `docs/reviews/<YYYY-MM-DD>-<subsystem>-rewrite.md`:

1. **Problem:** what is wrong now, with evidence: `file:line`, the derivation that shows the error, `trace-forensics` numbers, `control-auditor` findings, bench sessions from the ledger.
2. **Design:** the new law or structure from first principles, with units and frames explicit. Include its discrete-time behaviour at 200 Hz with one tick of delay, its saturation behaviour, and how it handles transitions (enable, retarget, mode and gain changes).
3. **Why it is the best option:** the realistic alternatives, including "patch the existing code", and why each one loses on correctness, robustness or simplicity.
4. **Predicted effect:** which bench criteria move and by how much, and what new risk the change brings.
5. **Verification plan:** the tests and the bench runs that will confirm the prediction.

A section you cannot fill means the conviction is not there yet. Investigate further rather than shrinking the change to fit the evidence you have.

## Doing it

1. **Baseline.** Record the current behaviour so the improvement is measured, not asserted: the relevant crate tests, the fitted-plant replay (`cargo test -p berthier bench_replay`), and the latest bench scores in `docs/commissioning/bench-ledger.md`.
2. **Replace rather than layer.** Remove superseded paths, flags and shims. Keep an old path only as a deliberate per-joint rollout switch (as ADR 0039 does with `law: legacy | scaled_pd`), and only with a stated condition for removing it.
3. **Test the new contract.** Keep every test that encodes a safety contract, and make it pass. Rewrite tests that pinned incidental behaviour. Add red → green tests for each defect the rewrite fixes (`bench-to-test`).
4. **Gate.** Iterate on the narrowest tests and climb the ladder in `test-speed` (crate, then dependents, then workspace, then `just check` once before merge), with clippy `-D warnings` and the replay against its baseline. A rewrite is also a chance to delete slow, real-time tests of the old structure and replace them with fast ones on simulated time.
5. **Documents.** Update `CONTEXT.md` terms and the relevant `docs/safety.md` sections. If the change departs from a decided design, write a superseding ADR (ADR 0039 governs the position law).
6. **Bench.** Deploy and run through `bench-run`: arm supported, reduced speed fractions first, then the full suite. Compare against the baseline ledger rows.

## Explaining it

The owner wants the full reasoning. Report:

- what changed and why it is better;
- the derivation;
- the alternatives and why they lost;
- the before and after numbers;
- the risks that remain.

Be direct. When the evidence is solid, say the old code was wrong and show why.
