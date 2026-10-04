# Control audit: shoulder-pitch slow-speed position law

Date: 2026-10-04. Scope: `right_shoulder_pitch`, 0.05–0.4 rad/s references, wave reversals and move endings. Baseline: deployed `02fcf7b4aa908d9b5e3cd9d0973fb8a9ba40c2f9`. Main source/config/URDF/tests were not changed; only this report was edited there during the worktree phase. Experiments are uncommitted in `/Users/joseph/code/marengo-wt/audit-pitch`, branch `audit/pitch-slow-speed`. No hardware action, installed gain/configuration change or URDF change was made.

## Conclusion

**The slow-wave endpoint struggle is real static friction; the continuing moving jitter is principally consistent with friction/ripple, not demonstrated firmware velocity quantization.** The assist fixes much of the endpoint error but barely improves measured moving-velocity smoothness. Lower drive `kd` makes true motion less smooth in the replay even though it makes torque look quieter.

Recommend one reduced-speed bench candidate: **drive PD `Kp=40 Nm/rad`, `Kd=5 Nm·s/rad`; continuous filtered host damping `H=2 Nm·s/rad`, filter 10 ms; reference-error integral `Ki=5 Nm/(rad·s)`, always leaking with 2 s time constant, cap 0.25 Nm; remove error-directed friction assist.** Keep current reference friction, URDF acceleration feed-forward, governor, speeds, acceleration, envelopes, caps and fuses. Add a Position-only whole-FF guard of **0.048 Nm per sent tick**, leaving quantization room below 0.05 Nm. This is a derived proposal, not permission to apply physical tuning.

The candidate passes the extended recorded-suite replay on all four prescribed plants and all three estimator hypotheses. It does **not** pass every wider uncertainty case: 12/81 cases exceed the fast-speed overshoot bar. It is not a claim of perfect motion or full qualification.

## First-principles specification (written before extracting the law)

Joint-space plant: `J(q) q̈ + C(q,q̇)q̇ + τ_g(q) + F(q,q̇) = τ_drive`, with q in URDF-axis rad, velocity in rad/s, J in kg·m², and all torques in Nm. For this one-joint slow-speed audit, assume rigid links, other joints held, ratio 1, locally constant effective J, negligible Coriolis, and a sampled host at T = 0.005 s with at least one sample of measurement/transport delay. Payload and firmware velocity estimator remain uncertain.

Pitch axis is URDF +Y (`assets/urdf/marengo.urdf:96–97`); world gravity is `(0,0,−9.81) m/s²` (`crates/armee-dynamics/src/urdf_gravity.rs:12`). With descendant CoM displacement r_i in world metres from the joint, unit world axis a, mass m_i in kg and rotated CoM inertia I_i in kg·m², compensation is `τ_g=−Σ a·(r_i×m_i g)=∂U/∂q`, and locked-descendant link inertia is `J_links=Σ[aᵀ I_i a+m_i|a×r_i|²]`. At the all-zero pose, A and B below come from the summed −9.81m z and −9.81m x moments. This derivation does not include unmodelled reflected rotor inertia or an added payload.

For reference r, v, a, e = r − q, expected torque is `τ_g(q) + J a + F̂(v) + Kp e + D(v − v̂) + z`. Kp has units Nm/rad, D Nm·s/rad, and z Nm. A friction model can have Coulomb/Stribeck velocity dependence and position ripple; static friction is set-valued at zero velocity, not a force that can be cancelled by an arbitrarily small smooth velocity command. A useful design must beat this dead zone without masking a gravity-model fault.

Requirements: bounded stable error dynamics including sampled delay; positive *net* incremental damping after negative Stribeck slope; estimator-noise torque small compared with friction/ripple and safety margin; continuous reference and bumpless finite-memory terms across reversals, retargets and enable; no integral accumulation when output saturates or the reference is envelope-clamped; gravity remains model-only; host feed-forward step ≤ 0.05 Nm/tick; all Davout caps/envelopes and existing fuse guarantees retained. Bench-score passing is necessary but not sufficient: measure position-derived velocity ripple and torque jitter during constant-speed motion as well as tracking and stopping.

## Extraction: what the deployed law actually computes

Source references are to the baseline, not shifted worktree line numbers. Crate-qualified source paths are relative to main `crates/`; bare `position_*.rs` paths mean `crates/berthier/src/`. Config/docs paths are relative to the main root. Let `r,v` be the advanced reference, `u` the final target, `q̂,dq_h` the latest published joint feedback, `e=r−q̂`, `E=u−q̂`, and `T=5 ms`.

1. **Input and timing.** `berthier/src/loop.rs:1645–1652` drains feedback then reads q; `1726` evaluates the full URDF gravity model at that pose; `1747–1751` reads joint velocity. `davout/src/feedback_consumer.rs:857–865` replaces raw drive velocity with `(q_n−q_prev)/(received_at_n−received_at_prev)`, except the first sample. Thus trace `dq` is a host position derivative, **not the drive servo's velocity estimate**. The tick sends, traces, then drains again (`loop.rs:1964–1973`); all rows here are consecutive ticks, but trace time is nominal ticks×5 ms, not a measured acquisition clock. Feedback age/transport/drive-estimator lag cannot be recovered from these CSVs alone.
2. **Gains and inertia.** Override > ramp > YAML feeds wire kp/kd (`loop.rs:1743–1746,1770–1791`). The acceleration inertia is computed once at the full arm's zero pose (`loop.rs:450–455,1777`), not fitted online. Master pitch: `Kp=18`, `Kd=5`, `Ki=5`, `e0=0.03 rad`, `e1=0.12 rad`, integral band 0.02 rad and leak 0.5 s, assist `λ=20 s⁻¹` (`config/control.yaml:97–147`). Independent URDF sum and Rust replay agree: `J_URDF=0.071253278 kg·m²`, `τ_g(q)=2.660964442 sin(q)+0.037832628 cos(q) Nm`, with the other joints at zero.
3. **Governor/reference.** If advancing toward u grows the lead, `s=clip[1−(|e|−e0)/(e1−e0),0,1]`; otherwise `s=1` (`position_law.rs:174–191`). It limits the reference speed, rather than multiplying a clock. Braking speed is `sqrt((aT)²/4+2a|u−r|)−aT/2`; velocity approaches the desired signed speed by at most `aT`, and position uses the new velocity (`197–262,295–309`). This is the documented later ADR refinement, not an erroneous omission of `ds/dt` from an acceleration formula.
4. **Friction and assist.** Define `M(v)=fc+(fs−fc) exp(−|v|/vb)`, `w(v)=exp(−|v|/vb)`. The actual dynamic target is

   `d*=M(v) tanh[k(v+w(v)λe)] + fv v + fo + J_URDF (v−v_prev)/T`.

   Values: `fc=0.3526 Nm`, `fs=0.65 Nm`, `vb=0.08 rad/s`, `k=15 s/rad`, `fv=fo=0`. Magnitude follows reference speed; error affects direction, not magnitude (`position_law.rs:90–102,324–349`). `d←d+clip(d*−d,±6T)` (`350–357`): maximum dynamic step 0.030 Nm/tick. A second, independent slew state computes `d_plain` with `λ=0`.
5. **Integral.** `z←clip(z+Ki E T,±0.5 Nm)` if `|E|<0.02 rad`; otherwise `z←z exp(−T/0.5 s)` (`position_law.rs:269–288,359–365`). There is **no leak inside the band**, no downstream-clamp feedback, and no decay conditioned on a velocity reversal. `τ_ff=τ_g(q̂)+d+z` (`367–371`). Storing z in Nm correctly avoids a gain-change multiplication step.
6. **Setpoint/fuses.** Position is reference clamped into the velocity-dependent envelope; velocity is v; drive kp/kd are constant after entry ramp (`position_hold.rs:1746–1758,1865–1872`). AscentStall and WaveStall keep measured-progress budgets. HoldTracking judges

   `Kp(q_des−q̂)+Kd(v−dq_h)+τ_ff−(d−d_plain)`

   (`1759–1805`). The assist is deliberately excluded. This correctly preserves the stated model-fault guarantee; removing that exclusion would be a safety regression.
7. **Davout.** It clamps the position envelope using the larger command/measured speed, evaluates danger zones, checks the resolved velocity cap, caps/rate-limits FF, then bounds predicted total MIT torque (`davout/src/lib.rs:2990–3100`). For pitch: cap 5 Nm; rate limit 60 Nm/s, normally 0.30 Nm/tick, **not** the 0.05 Nm motion-quality bar. The prediction is

   `|P+D+τ_ff| + Kp max(|dq_h|,|v_des|)·0.010 s + Kd·0.1 rad/s`.

   An over-cap request scales its position/velocity errors toward feedback by one common reachable factor; kp/kd/FF are unchanged (`total_torque.rs:91–104,140–180`). It is a host prediction, not a certified bound on unknown drive-estimator error.
8. **Motor/wire.** Pitch direction is −1 and external ratio 1 (`config/motors.yaml:8–21`). Davout sends `q_m=−q`, `v_m=−v`, `τ_m=−τ`, unchanged gains; the general transform divides gains by ratio² (`lib.rs:3261–3280,3337–3359`). Internal drive gearing is already represented by vendor output-space units; applying 9 again would be wrong. RS03 signed fields are rounded about code 32767 (`robstride/src/mit.rs:53–73,127–139`). Position step 0.383507 mrad, velocity field step 0.000610370 rad/s, FF step 0.001831111 Nm; gain steps 0.076295 Nm/rad and 0.00152590 Nm·s/rad. **A feedback-position derivative quantum of 0.0767 rad/s is not the MIT velocity-field quantum.** At recommended gains, position/velocity/FF rounding contribute roughly 0.010 Nm worst-case combined error before gain rounding; they cannot explain a 0.2–0.4 Nm disturbance alone.

## Measurements and attribution

Archives are `var/motion-suite/<timestamp>/`, each with trace, plan, bench log, captured config/URDF and score. Captured configs have no assist in the first two sessions and λ20 in the subsequent six. All eight pitch traces have 100% consecutive-tick coverage. The bundled `trace-forensics/scripts/decompose.py --json` was run on all eight; outputs are in main `var/control-audit/2026-10-04-pitch-slow-speed/`. Its simple linear inertia/friction regression is **not** ground truth: evening wave fits even return negative inertias, and mixed-speed up/down bins confound Stribeck, acceleration and ripple. No gain increase is justified by interpreting those contaminated bins as a proven gravity error.

| Session UTC | Actual deciding evidence | Verdict |
|---|---|---|
| 20261004T202431Z | Slow-wave tracking 42.8 mrad; no total clamp | FAIL tracking |
| 20261004T202646Z | Slow-wave tracking 46.7 mrad; ticks 10807–11705 remain >30 mrad behind near rest for 4.495 s | FAIL tracking |
| 20261004T211711Z | With assist, worst slow-wave tracking 28.2 mrad | PASS |
| 20261004T211926Z | Slow-wave tracking 14.3 mrad; one total clamp | FAIL clamp |
| 20261004T212243Z | Slow-wave tracking 15.9 mrad; one total clamp | FAIL clamp |
| 20261004T212654Z | Stops 12.655 and 14.189 mrad, against 10 mrad bar | FAIL stopping |
| 20261004T212824Z | Stops 10.507 and 10.891 mrad | FAIL stopping |
| 20261004T212907Z | Five return stops 10.738–11.121 mrad; final repeat spread essentially zero | FAIL stopping |

The moving smoothness barely changes when endpoint tracking improves: same broad sweep before/after (`202646Z` / `211926Z`), RMS of 50 ms Savitzky–Golay position-derived velocity minus reference, on `0.05≤|v_ref|≤0.4 rad/s`, is **0.05540 / 0.05499 rad/s**. That is only a 0.7% reduction. A slow-wave speed excursion of +67% remains explicitly “not scored” in the latter's `score.txt:7`. Passing slow tracking does not mean moving smoothly.

### Test of the drive-damping hypothesis

Fit, on that same velocity-reference band,

`τ_meas = α [τ_p + Kd v_des + τ_ff_wire] − D v_est + c`.

| Trace | With trace host `dq`: RMS Nm | With smooth 50 ms velocity: RMS Nm | Smooth fit α / D (Nm·s/rad) |
|---|---:|---:|---:|
| 202431Z | 0.21694 | 0.11332 | 0.9992 / 5.0849 |
| 202646Z | 0.21365 | 0.10818 | 0.9975 / 5.0484 |
| 211711Z | 0.21700 | 0.11456 | 0.9997 / 5.0851 |
| 211926Z | 0.21425 | 0.11092 | 0.9969 / 5.0367 |
| 212243Z | 0.22111 | 0.11807 | 0.9963 / 5.0257 |

Smooth velocity explains the measured torque approximately twice as well. Trace `dq` minus smooth velocity has RMS 0.0361–0.0367 rad/s in these moving bands, equivalent to **hypothetical** kd5 torque noise 0.181–0.183 Nm. This is not measured firmware damping noise. The measured torque estimator has its own delay/bias; these regressions do not identify the exact internal estimator. They do contradict using trace count toggles as proof that firmware damping applies those same toggles.

A richer moving-row fit, `τ−τ_g = J a + fc sign(v) + Δfs sign(v) exp(−|v|/0.08) + R_s sin(2π·28.6q)+R_c cos(2π·28.6q)+ΔA sin(q)+ΔB cos(q)+c`, gives measured ripple amplitudes **0.196–0.242 Nm** in the five wave sessions, unexplained RMS about 0.15–0.16 Nm. This is evidence of position-dependent resistance, not a perfectly identified gearbox model. The replay's 0.4 Nm ripple is a conservative mechanism-fitting amplitude, particularly for breakaway extremes. The competing torque sources yield substantially different inertias; retain that uncertainty rather than “calibrating” rotor inertia from one regression.

## Findings, ordered by severity

### Wrong — the replay/documentation equates different velocity estimates

**Expected:** distinguish wire-reported velocity, host inferred velocity and internal drive damping velocity. **Implemented assertion:** original `bench_replay.rs:18–19` claims measured torque follows a one-count/5 ms estimator better than smooth velocity; ADR 0039 `56–58` says the host uses the same estimate as the drive. Davout `feedback_consumer.rs:857–865` shows this is false for established samples. The regression above gives 0.214 versus 0.111 Nm RMS in `211926Z`, contrary to the replay comment. A 0.3835 mrad position count differentiated over 5 ms gives 0.0767 rad/s; it must not be mistaken for the 0.0006104 rad/s wire velocity step.

**Correction:** corrected the experiment's comment, explicitly varied 5 ms count / 20 ms count / ideal drive velocity, and retained kd5 pending real estimator evidence. Supersede the ADR's incorrect evidence sentence; do not throw away its sound constant-drive-damping rationale. **Regression:** preserve raw velocity alongside inferred velocity in a wire/feedback fixture; show that changing inference cannot change what a firmware velocity field contained. **Tuning:** documentation/test correction needs no physical tuning; reducing kd on this mistaken premise does and is rejected. No deployed torque sign/unit error was found here.

### Fragile — “friction error assist” is a substantial nonlinear feedback gain, not measured stiction cancellation

**Expected:** compensate nominal motion friction and use an explicitly justified feedback law for residual error. **Actual:** the reference-only rest weight is applied whether the joint is stuck or moving (`position_law.rs:324–340`). At v=0, the added rest torque is `fs tanh(kλe)`. Its small-error stiffness is **fs·k·λ=195 Nm/rad**, in addition to kp18. At `v=0.05 rad/s`, e=0.01 rad increases friction FF by about **0.188 Nm**, comparable with the original P torque 0.18 Nm.

The assist was rationally added to beat the measured dead zone: on the 1.05 Nm crest, rest breakaway solves `18e+0.65tanh(300e)=1.05`, giving **22.22 mrad**, versus **58.33 mrad** for P alone. This explains the large tracking improvement. It does not establish smoothness. For a sliding local stress linearization with J0.14 and worst friction slope −3.5 Nm·s/rad, net damping is only 1.5; raising local stiffness from 18 to 213 reduces continuous damping ratio from about **0.47 to 0.14**. Including ZOH and one extra old measurement, the simple unsaturated assist model has pole modulus **0.99864**, versus **0.97321** without it. This is weak damping, **not proof that the actual stick-slip arm is unstable**; static friction and slew invalidate a small-signal model at exact rest.

**Evidence:** endpoint error improves; moving RMS does not. The extended replay shows deployed final-second hold excursions up to **18.79 mrad** under the 20 ms estimator; its parked stall gate still fails slow tracking, up to **38.62 mrad** on fitted variants / **34.88 mrad** on the crest. The gate counts 4 onset + 4 ramp ticks: 20 + 20 ms, not the patch comments' “50 ms” each. Reconstructed `212907Z` deceleration assist often **opposes** the motion; it is not defensible to attribute all five stopping failures to a continuously downward assist. The replay does not reproduce every 14.2 mrad stop. The stopping causal attribution therefore remains partial.

**Correction:** remove this added stiffness and choose a moderately higher explicit kp with continuous modest extra damping, as the single candidate below. Do not stack more stall/onset recipes. Keep assist-excluding fuse accounting on any retained rollout path. **Regression:** extended slow crest waves, final-second hold excursion and stopping, red on baseline/green on candidate; plus an independent moving-joint no-assist test if the gated alternative is ever revived. **Tuning:** yes; bench gravity verification first, then supported reduced-speed confirmation. An actual HoldTracking trip at home still means fix the model, never increase gains to silence it.

### Fragile — target-band integral can preserve and accumulate a stale torque through motion and reshaping

**Expected:** a bounded disturbance estimate tied to the controlled reference, with explicit decay and saturation handling. **Actual:** integration uses final target error and leaks only outside its band (`position_law.rs:269–288`); no Davout clamp/envelope feedback enters it. At the band edge it can accumulate **0.1 Nm/s**, reaching the 0.5 Nm cap in 5 s. That stored torque is equivalent to **27.8 mrad** of kp18 error. One wave trace (`211926Z`) reaches **0.4016 Nm**, not a negligible trim. A 10 mrad static error adds 0.05 Nm/s inside the band, while the state never leaks there.

The ADR deliberately narrowed the band to avoid the former 0.1 rad window's overshoot and stored z in torque for bumpless gains. Both are good choices; neither solves indefinite in-band accumulation or downstream reshaping. On the replay, rest-only leak improves stopping but not moving smoothness; reference-error leak improves crest tracking, but kp50 retains clamps and the same velocity ripple. These are useful components, not a standalone solution.

**Correction:** `z_next=clip(exp(−T/2s)z + 5·e·T, ±0.25 Nm)` when `|e|<0.02 rad` and output was not reshaped; otherwise leak only. No target-distance integration while the reference is elsewhere. Nominal constant error then produces `z≈Ki·2s·e`, an additional low-frequency stiffness of 10 Nm/rad, not an unbounded ramp. **Regression:** block output while error remains in band and verify decay, not growth; reverse/retarget without retaining a saturated push; apply an envelope clamp and a total clamp. The test-only candidate explicitly freezes input after downstream shaping; its direct blocked-integral test passes. **Tuning:** yes, including integral cap and leak. Full lifecycle/physical antiwindup tests remain necessary before implementation is selected in production.

### Imprecise — friction model, ripple and effective inertia remain materially uncertain

**Expected:** distinguish physical sliding friction from the smoothed FF used to avoid torque steps. **Actual:** reference FF is intentionally smooth, zero at zero velocity. Against the fitted plant `F(v)=0.37+0.28exp(−|v|/0.08) Nm`, it undercompensates by **0.1948 Nm at 0.05 rad/s**, **0.05394 Nm at 0.1**, and **0.01784 Nm at 0.2**. At kp18 these correspond to **10.82, 3.00, 0.99 mrad** before ripple. At v→0 the static 0.65 Nm is set-valued; `tanh(0)=0` is not a mistaken gravity sign.

Ripple at 28.6 cycles/rad is excited at **1.43–11.44 Hz** across 0.05–0.4 rad/s. This is well below the 100 Hz Nyquist frequency; it is not a 200 Hz aliasing explanation. At 0.14 rad/s its 4.0 Hz forcing is near/above the kp18/J0.14 natural frequency **1.80 Hz**. Measured smooth-velocity ripple remains about 0.055 rad/s even after assist improves tracking. A calibrated positional-ripple feed-forward could help, but phase/amplitude are not stable enough here to justify a map: measured wave-session fits range roughly 0.20–0.24 Nm and change phase; treating an unverified map as truth risks adding torque in the wrong place.

The plant's effective J0.14 versus URDF0.071253 gives **0.10312 Nm** missing acceleration FF at 1.5 rad/s², or **5.73 mrad** at kp18. This is an estimate, not a proved URDF defect: commanded/measured torque give conflicting inertia, and the real five-link pose changes. Candidate trials using Jff0.14 improve some stops by a few mrad but do not improve slow velocity RMS and can consume clamp headroom. **Correction:** keep current inertia for the first trial; identify it separately with bidirectional moderate-acceleration moves and a smooth/raw drive-velocity comparison. **Regression:** vary J0.10–0.18, ripple phase and estimator rather than overfitting one stop. **Tuning:** friction, inertia FF and a ripple map all need bench evidence; none was changed.

### Unclear — unconditional “no host damping” is an architecture choice, not a measured optimality result

ADR 0039 `81–83,109–110` intentionally removed the old gated, delayed host torque path and accidental double damping. That was justified: its deadband released whole torque pushes and its estimator lag was uncontrolled. It does **not** prove that a small, always-on, filtered auxiliary damper is inferior. The candidate here intentionally sums two known dampers, with a 10 ms filter and one-sample delay model; it does not reinstate the old deadband, onset switch, overspeed brake cap or equal-gain duplication. This departure needs an ADR revision before rollout. Its measured/simulated advantage is about **21% reduction in true slow velocity-error RMS**, not elimination of jitter. Physical confirmation is still the deciding evidence.

## Experiments: fitted plants, scoring, and alternatives

The worktree extends the **existing Rust fitted-plant replay**, not just an independent Python simulator. It runs production reference generation, retargets, descent cap and all three fuses. Candidate FF is injected **before** the actual fuse calculation through a `#[cfg(test)]` seam; baseline paths are untouched. This matters: an earlier experimental version patched torque after fuses and was insufficient as fuse evidence. The final runs below use the corrected seam. The qualification/uncertainty candidate additionally guards the complete FF step at 0.048 Nm; the alternative gain sweeps retain their stated limits.

Plant: J0.14 kg·m², fc0.37/fs0.65 Nm, vb0.08 rad/s, sinusoidal ripple 0.4 Nm at 28.6 cycles/rad, URDF gravity. Fitted variants scale both gravity coefficients by 0.96 and 1.04. Crest: fs1.05 Nm, no ripple, same J/fc/vb/gravity. Encoder feedback has one-count-wide noise; mechanical integration uses 20 substeps/tick. The three drive estimators are count difference over 5 ms, count difference over 20 ms, and exact true velocity. None is claimed to be measured firmware behavior.

Final experiments add one extra old feedback sample, reproduce Davout's torque prediction/common-factor reshape, danger-zone velocity clamp and FF limiter, and use **actual `robstride::encode_mit` / decoded fields**, including pitch's −1 sign, f32 conversion and gain quantization. The delay FIFO was separately tested not to create a spurious two-tick position derivative. No drive-side 5 Nm clamp is assumed authoritative: the historical plant retains that numerical saturation, so over-cap candidates are rejected rather than declared safe because the simulated drive clipped them.

Sessions: existing long/short moves, sweeps, midflight reversals, gravity extremes and five repeats, plus extended stops at 0.1/0.496/1.55 rad and extended waves through −1.038 to 2.533/2.930 rad. It is a mechanism replay of recorded ranges/timings, not an exact event-by-event reproduction of the archived suites.

Metrics: slow maximum `|q−r|` (≤30 mrad); fast 50 ms speed excess (≤20%); maximum stop excursion (≤10 mrad); larger of pre-filter within-move and decoded-wire FF steps (≤0.05 Nm); total-clamp count (0). Smoothness `V` is **RMS(true plant velocity−reference velocity)** on all rows with `0.05≤|v_ref|≤0.4 rad/s`, including ramps/turnarounds in that band. Torque jitter `Q` is RMS intra-tick motor-torque deviation from its tick mean on those rows. It is not a measurement from the arm. Hold excursion is final-second position range only when the reference is stationary; unfinished moves/wave tails are excluded. Repeat spread uses only the dedicated repeat session, not arbitrary returns after unrelated moves.

### Recommended candidate: all prescribed plants

Numbers are predictions, not measured scores. “Drive” denotes the estimator hypothesis. All rows have **0 clamps, 0 faults, hold excursion ≤0.384 mrad, repeat spread ≤0.384 mrad**.

| Drive | Plant | Slow mrad | Fast excess % | Stop mrad | FF step Nm | V rad/s | Q Nm | Max predicted total Nm |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 5 ms | fitted | 21.86 | 14.87 | 3.54 | .04800 | .04415 | .15142 | 4.85829 |
| 5 ms | gravity×.96 | 22.32 | 14.87 | 5.91 | .04800 | .04413 | .15130 | 4.60901 |
| 5 ms | gravity×1.04 | 21.87 | 14.87 | 2.47 | .04800 | .04413 | .15141 | 4.93997 |
| 5 ms | crest | 22.07 | 2.82 | .52 | .04800 | .01574 | .15440 | 4.95358 |
| 20 ms | fitted | 21.79 | 19.87 | 2.30 | .04800 | .04684 | .04086 | 4.78541 |
| 20 ms | gravity×.96 | 22.01 | 19.87 | 4.31 | .04800 | .04684 | .04091 | 4.62987 |
| 20 ms | gravity×1.04 | 21.87 | 19.87 | 3.62 | .04800 | .04680 | .04090 | 4.97157 |
| 20 ms | crest | 22.26 | 2.39 | .00 | .04800 | .01594 | .03938 | 4.87314 |
| ideal | fitted | 21.87 | 14.87 | 3.92 | .04800 | .04374 | .01368 | 4.86339 |
| ideal | gravity×.96 | 22.32 | 14.87 | 6.67 | .04800 | .04373 | .01370 | 4.67826 |
| ideal | gravity×1.04 | 22.25 | 14.87 | 2.30 | .04800 | .04375 | .01367 | 4.96328 |
| ideal | crest | 22.26 | 2.82 | .90 | .04800 | .01575 | .01051 | 4.94055 |

Nominal fitted 5 ms comparison: deployed V .05576 → candidate .04415 rad/s (**20.8% lower**); Q .15015 → .15142 Nm (**not lower**). Ideal-estimator fitted comparison: deployed V about .0549 → .0437 rad/s. The advantage is not contingent on believing the coarse-estimator hypothesis. Neither gain-only changes nor quieter intra-tick torque are reliable proxies for smoother motion.

### Rejected alternatives

`P/D/H` denote explicit stiffness/drive damping/host damping. No assist unless stated. Plain gains-only candidates retain production integral and URDF Jff. H4 uses a 15 ms filter; restI leaks in 0.5 s and integrates target error only near rest; refI leaks in 2 s and integrates reference error, both with 0.25 Nm cap. Final `ff14` trials substitute Jff0.14. F = **worst per metric across nominal and both ±4% fitted plants, and all three drive estimators**; C = worst across estimators on the crest. Clamp counts are maximum per run, not summed over repeated uncertainty runs. Full per-plant rows are preserved in worktree `var/control-audit/{candidates,final,refinement}.txt`.

| Alternative | Plant | Slow mrad | Fast % | Stop mrad | FF Nm | Clamps | V rad/s | Q Nm |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Deployed kp18/kd5/λ20 | F | 31.85 | 17.37 | 10.89 | .03662 | 0 | .05971 | .15017 |
| Deployed kp18/kd5/λ20 | C | 30.73 | 4.36 | 3.64 | .03662 | 0 | .02269 | .15370 |
| Plain kp18/kd5/λ0 | F | 51.58 | 17.37 | 12.04 | .03715 | 0 | .06028 | .15002 |
| Plain kp18/kd5/λ0 | C | 42.59 | 2.47 | 2.82 | .03662 | 0 | .02138 | .15177 |
| Parked stall-gated assist | F | 38.62 | 17.37 | 12.04 | .03662 | 0 | .06015 | .15013 |
| Parked stall-gated assist | C | 34.88 | 4.88 | 8.59 | .03662 | 0 | .02360 | .15276 |
| P50D3 | F | 21.75 | 19.87 | 10.89 | .03662 | 0 | .08354 | .08430 |
| P50D3 | C | 23.88 | 2.82 | 2.11 | .03662 | 0 | .03816 | .09143 |
| P50D5 | F | 21.95 | 17.37 | 8.21 | .03662 | 1 | .05779 | .15014 |
| P50D5 | C | 23.44 | 2.82 | 3.22 | .03662 | 0 | .01671 | .15476 |
| P72D5 | F | 16.80 | 19.87 | 5.91 | .03662 | 9 | .05468 | .15209 |
| P72D5 | C | 18.25 | 2.82 | 1.30 | .03662 | 2 | .01512 | .15548 |
| P50D8 | F | 22.03 | 14.87 | 5.52 | .03846 | 200 | .03859 | .24014 |
| P50D8 | C | 22.95 | 2.82 | 2.49 | .03662 | 27 | .01214 | .24687 |
| P72D8 | F | 16.42 | 14.87 | 4.69 | .03713 | 350 | .03727 | .24148 |
| P72D8 | C | 17.78 | 2.82 | 1.34 | .03662 | 128 | .01094 | .24773 |
| P100D8 | F | 12.03 | 14.87 | 3.92 | .03662 | 906 | .03635 | .24222 |
| P100D8 | C | 13.30 | 2.39 | .54 | .03662 | 619 | .01245 | .24886 |
| P120D8 | F | 10.03 | 17.37 | 3.54 | .03714 | 1810 | .03691 | .24231 |
| P120D8 | C | 11.09 | 2.39 | .15 | .03662 | 1311 | .01497 | .24967 |
| P150D8 | F | 8.14 | 17.37 | 3.16 | .03846 | 3375 | .04096 | .24252 |
| P150D8 | C | 8.82 | 2.82 | .00 | .03846 | 2829 | .02461 | .25109 |
| P72D10 | F | 16.09 | 14.87 | 4.31 | .03846 | 1374 | .03091 | .30096 |
| P72D10 | C | 17.39 | 4.88 | 1.34 | .03662 | 896 | .00944 | .30953 |
| P100D10 | F | 11.90 | 14.87 | 3.54 | .03714 | 2498 | .03111 | .30184 |
| P100D10 | C | 13.16 | 2.39 | .19 | .03662 | 1764 | .01267 | .31057 |
| P50D2H4 | F | 21.95 | 34.85 | 11.27 | .04761 | 0 | .07754 | .05909 |
| P50D2H4 | C | 23.17 | 4.88 | 6.13 | .04761 | 0 | .07080 | .06159 |
| P72D2H4 | F | 16.36 | 44.68 | 10.51 | .04761 | 0 | .07637 | .06160 |
| P72D2H4 | C | 18.06 | 9.58 | 6.14 | .04761 | 0 | .07502 | .05807 |
| P50D5restI | F | 21.09 | 19.87 | 6.23 | .03662 | 1 | .05764 | .15021 |
| P50D5restI | C | 20.72 | 2.39 | .00 | .03662 | 0 | .01638 | .15487 |
| P50D5refI | F | 18.69 | 17.37 | 7.06 | .03662 | 2 | .05775 | .15013 |
| P50D5refI | C | 17.66 | 2.82 | 1.30 | .03662 | 0 | .01659 | .15492 |
| P36D5H2refIff14 | F | 24.22 | 19.87 | 5.54 | .04761 | 0 | .04724 | .15147 |
| P36D5H2refIff14 | C | 26.01 | 3.46 | .00 | .04761 | 0 | .01693 | .15413 |
| P40D5H2refIff14 | F | 22.32 | 19.87 | 4.77 | .04761 | 0 | .04700 | .15167 |
| P40D5H2refIff14 | C | 23.03 | 3.46 | .00 | .04761 | 0 | .01670 | .15429 |
| P44D5H2refIff14 | F | 20.72 | 19.87 | 4.39 | .04761 | 1 | .04675 | .15189 |
| P44D5H2refIff14 | C | 20.35 | 2.82 | .00 | .04761 | 1 | .01635 | .15451 |
| P40D5H1.5refIff14 | F | 22.40 | 19.87 | 4.77 | .04761 | 1 | .04890 | .15213 |
| P40D5H1.5refIff14 | C | 23.22 | 4.36 | .00 | .04761 | 0 | .01686 | .15429 |
| P40D5refIff14 (H0) | F | 22.78 | 17.37 | 5.14 | .03722 | 1 | .05899 | .14996 |
| P40D5refIff14 (H0) | C | 23.03 | 2.82 | .00 | .03662 | 0 | .01789 | .15439 |
| P40D3H3refIff14 | F | 22.25 | 24.86 | 6.69 | .04761 | 0 | .06210 | .09102 |
| P40D3H3refIff14 | C | 22.84 | 3.46 | .15 | .04761 | 0 | .02181 | .09283 |
| P40D3H4refIff14 | F | 22.23 | 24.86 | 7.46 | .04761 | 0 | .06270 | .09155 |
| P40D3H4refIff14 | C | 22.65 | 4.88 | 1.53 | .04761 | 0 | .02265 | .09312 |
| P40D4H3refIff14 | F | 22.23 | 24.86 | 5.93 | .04761 | 0 | .05202 | .12154 |
| P40D4H3refIff14 | C | 22.84 | 4.36 | .00 | .04761 | 0 | .01878 | .12378 |
| P36D3H4refIff14 | F | 23.68 | 24.86 | 7.46 | .04761 | 0 | .06311 | .09137 |
| P36D3H4refIff14 | C | 25.52 | 4.88 | 1.14 | .04761 | 0 | .02267 | .09304 |

Smaller-gain refinements (same F/C aggregation; H2 filter 10 ms, H3 filter 5 ms; URDF Jff):

| Alternative | Plant | Slow mrad | Fast % | Stop mrad | FF Nm | Clamps | V rad/s | Q Nm |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| P44D5 | F | 23.78 | 17.37 | 8.97 | .03846 | 1 | .05848 | .14994 |
| P44D5 | C | 25.34 | 4.88 | 3.99 | .03662 | 0 | .01733 | .15456 |
| P44D5refI | F | 20.72 | 17.37 | 7.82 | .03846 | 0 | .05844 | .14974 |
| P44D5refI | C | 21.16 | 2.82 | 1.30 | .03662 | 0 | .01721 | .15468 |
| P36D5H2 | F | 27.94 | 19.87 | 6.67 | .04761 | 0 | .04713 | .15133 |
| P36D5H2 | C | 27.61 | 2.82 | 2.45 | .04761 | 0 | .01649 | .15403 |
| P44D5H2 | F | 23.95 | 22.37 | 5.91 | .04761 | 0 | .04664 | .15169 |
| P44D5H2 | C | 24.75 | 4.88 | 2.49 | .04761 | 0 | .01590 | .15435 |
| P44D5H2refI | F | 20.72 | 19.87 | 5.91 | .04761 | 0 | .04660 | .15175 |
| P44D5H2refI | C | 20.73 | 4.36 | .52 | .04761 | 0 | .01565 | .15456 |
| P44D5H3 | F | 23.78 | 22.37 | 5.52 | .04761 | 0 | .04517 | .15101 |
| P44D5H3 | C | 24.48 | 4.88 | 1.73 | .04761 | 1 | .01580 | .15496 |
| P44D5H3refI | F | 20.72 | 22.37 | 5.91 | .04761 | 0 | .04515 | .15103 |
| P44D5H3refI | C | 20.35 | 4.36 | .52 | .04761 | 0 | .01559 | .15524 |

Plain kp18 also faults on some prescribed variants/crest sessions, so its partial scores are not qualification. Deployed hold excursion reaches 17–19 mrad; most non-assist candidates reduce it to one count. The kp36/44 refinements establish the trade-off rather than a separate recommended law: kp36 has less slow tracking margin, kp44 without H does not improve smoothness, and adding H3 buys little V improvement but loses fast overshoot margin. Kp44/H2/refI improves tracking by about 1.6 mrad over kp40 at nearly identical V; prefer the lower explicit stiffness and more torque-margin reserve for the first trial, not a claim of uniquely optimal gains. No candidate with repeated clamps is acceptable merely because tracking is excellent. Lower drive damping loses on true V and fast speed, even when Q improves. Higher drive damping wins V but consumes the total-torque margin and raises Q. Larger kp alone closes the dead zone but barely improves V. The selected kp40/H2/reference-I balances those effects without relying on a newly fitted inertia or ripple map.

### Delay, stability, saturation and numerical checks

For sliding error dynamics with friction slope F′, the ideal local law is `J ë+(Kd+F′)ė+Kp e=0`. Plant worst slope is `−(0.65−0.37)/0.08=−3.5 Nm·s/rad`; kd3 is insufficient at that edge, kd5 leaves positive damping. The Python experiment models local drive PD continuously, not incorrectly as a delayed host PD; discretizes it by matrix exponential/ZOH; and includes old host position samples, position-difference EMA and always-leaky reference integral. Recommended Kp40/kd5/H2/J0.14 with worst F′ and one old sample has poles **0.996841, 0.906806±0.046236i, 0.755553** (remaining memory poles smaller). With two old samples the largest modulus is **0.996839**. These are local unsaturated stability checks, not a global nonlinear/safety proof. Integral cap, dynamic slew, stick-slip and governor are exercised by the nonlinear replay.

Uncertainty grid: estimator {5 ms,20 ms,ideal} × J {0.10,0.14,0.18} × ripple phase {0,π/2,π} × extra old samples {0,1,2}, **81 cases**. Worst slow track **25.07 mrad**, stop **6.29 mrad**, FF step **0.04800 Nm**, V **0.05054 rad/s**, hold excursion **0.384 mrad**, repeat spread **0.77 mrad**, **zero clamps and faults**. But **12 cases fail fast overshoot**, worst **27.36%** at J0.10, 20 ms drive estimator, phase π/2, two old samples. Nominal 19.87% has almost no fast-score margin. Do not proceed directly to full-speed qualification.

Timing jitter is not settled by these runs: Berthier uses nominal planner time; Davout infers velocity from receive intervals and limits FF with elapsed time capped at 10 ms (`lib.rs:3313–3319`). At 4/6 ms, one feedback-position count is 0.0959/0.0639 rad/s, rather than 0.0767. The candidate replay uses fixed 5 ms. Production filtering must use fresh timestamped position differences, must not differentiate a held sample as a new sample, and must reject nonfinite inputs. Existing configuration and period guards reject invalid gains/periods; standalone pure-law fallback-to-zero behavior is not a substitute for those guards. Variable acquisition/tick jitter and mode/gain transitions need explicit tests before rollout. Dynamic slew alone cannot guarantee a 0.05 Nm whole-FF step after an unexpected measured-pose change; the candidate therefore also limits the complete Position FF request to 0.048 Nm/tick. Adjacent signed torque quantization adds at most 0.0018312 Nm, leaving a decoded-wire bound below 0.05 Nm. Its abrupt-pose unit test passes. Initialize that guard from the last actual sent FF on mode entry, not zero on a supported elevated transition.

## Verification and reproducibility

**Files changed/added in the experiment worktree (uncommitted):**

- `crates/berthier/src/position_hold_tests/bench_replay.rs`: estimator alternatives, delay FIFO, actual wire quantization, total-torque reshaping, smoothness/torque/hold/repeat metrics, test-only candidate FF/integrals, matrices and red/green qualification.
- `crates/berthier/src/position_hold.rs`: 11-line `#[cfg(test)]` candidate seam before the unchanged fuse logic. No production selector or installed path changes.
- `control_audit_pitch.py`: experiment-only reproducible trace regressions, ripple fit, breakaway roots and sampled poles using numpy/scipy/sympy; deliberately outside production `scripts/`' stdlib diagnostic contract.
- `docs/test-timing.md`: measured narrow-test/matrix costs and why long diagnostics stay opt-in.

The parked `2026-10-04-stall-gated-assist.patch` is untouched; its mechanism was tested without applying its source changes. `var/control-audit/` contains raw experiment outputs, including superseded `*-before-fidelity.txt` exploratory runs: **only un-suffixed final outputs are used in the tables**.

Exact commands, each run with working directory `/Users/joseph/code/marengo-wt/audit-pitch`:

```sh
cargo check -p berthier --tests
cargo test -p berthier --lib bench_replay -- --nocapture
cargo test -p berthier --lib law_gates -- --nocapture
cargo test -p berthier --lib pitch_audit_candidate_matrix -- --ignored --nocapture
cargo test -p berthier --lib pitch_audit_refinement_matrix -- --ignored --nocapture
cargo test -p berthier --lib pitch_audit_final_matrix -- --ignored --nocapture
MARENGO_AUDIT_BASELINE=1 cargo test -p berthier --lib pitch_audit_recommended_law_meets_extended_bar -- --ignored --nocapture
cargo test -p berthier --lib pitch_audit_recommended_law_meets_extended_bar -- --ignored --nocapture
cargo test -p berthier --lib pitch_audit_uncertainty_grid -- --ignored --nocapture
cargo test -p berthier --lib
cargo clippy -p berthier --tests -- -D warnings
uv run --no-project --with numpy --with scipy --with sympy python3 \
  /Users/joseph/code/marengo-wt/audit-pitch/control_audit_pitch.py \
  --traces /Users/joseph/code/marengo/var/motion-suite
```

Results: deployed counterfactual **RED** (slow crest/gravity variants, hold excursion, stops); candidate **GREEN**, final qualification 4.12 s execution. This is red→green on the fitted mechanism, not on a changed production rollout. Final Berthier library suite **236 PASS, 5 diagnostic matrices ignored**, 0.53 s execution / 1.36 s wall including 0.52 s rebuild; clippy and formatting PASS. The final normal replay adds three tiny checks and retains all three original contracts (baseline execution approximately 0.48 s). Opt-in candidate/final/uncertainty matrices take about 69/43/28 s; they are intentionally excluded from default tests and justify their cost by scoring whole recorded suites across hypotheses. No workspace/merge gate was run for this audit-only test seam.

**Regression tests still required for production selection:** timestamped/dropped/repeated feedback and 4–6 ms jitter; bumpless enter/exit/retarget/gain ramp with the candidate's velocity/dynamic/integral states initialized consistently; clamp/envelope/integrator coupling through real Davout/SimulationBus; actual decoded-wire FF step through a whole transition; and a wrong-gravity-sign/home-sag fixture that trips HoldTracking with assist excluded. Current fuse/reference/mode library tests remain green, but a test-only singleton replay seam is not that full rollout.

## What is verified correct / what remains unverified

**Verified:** torque terms are in joint Nm; gains and direction/ratio transform are dimensionally and energetically correct for pitch; gravity uses measured pose with the right restoring-sign convention; actual wire ranges/quantization are not a 2.5× velocity-scale error here; constant drive kd avoids the old gating; reference retargets obey bounded acceleration; dynamic FF slew is 0.03 Nm/tick; the deliberate plain-law fuse torque excludes assist; total-torque prediction/reshape equations and descent cap are retained. No demonstrated sign, rad/degree, Nm/current, integration-scale or drive-gain-unit mistake in the deployed pitch law was found. All specified torque terms are accounted for; slow one-joint Coriolis and positional ripple remain unmodelled by production FF.

**Not verified:** live internal drive estimator, torque-estimator transfer/bias, actual sample age/jitter, changing coupled five-link inertia/payload, exact cause of all 14.2 mrad stops, mechanical compliance/backlash, a phase-accurate ripple map, physical E-stop wiring, or hardware safety/qualification. Local archives contain no candump for these eight runs; no live Pi read tool was available in this session. The replay does not invoke the full Davout owner, hard-limit faults or second velocity-based envelope pass: it reproduces torque/danger shaping and retains the existing independent safety tests. Its historical assumed drive torque saturation is not evidence of a firmware guarantee. These limits prevent declaring the arm “fixed” from a green replay.

## One recommended law and reduced-speed deciding experiment

Keep the current reference/governor. For measured pose q̂, a timestamped filtered velocity v̂ and reference r/v/a, use:

```text
e = r − q̂
v̂ ← v̂ + [1−exp(−T/0.010 s)] (Δq̂/Δt_sample − v̂)
d* = (0.3526 + 0.2974 exp(−|v|/0.08)) tanh(15v)
     + J_URDF a + 2(v−v̂)                         [Nm]
d ← d + clip(d*−d, ±6T)                          [Nm]
z ← clip(exp(−T/2s)z + 5 e T, ±0.25 Nm)           if |e|<0.02 rad and output not reshaped
z ← exp(−T/2s)z                                  otherwise
τ_ff* = τ_g(q̂)+d+z
τ_ff ← τ_ff_previous + clip(τ_ff*−τ_ff_previous, ±0.048 Nm)
MIT: q_des = envelope(r), v_des = v,
     kp = 40 Nm/rad, kd = 5 Nm·s/rad, torque_ff = τ_ff
```

**λ=0; no stall gate, error-friction stiffness, deadband switch or empirical ripple map.** Keep current `J_URDF` (zero-pose pitch 0.071253278 kg·m²), reference speed/acceleration, torque/velocity caps and 6 Nm/s dynamic slew. The complete Position FF guard is nominally 9.6 Nm/s but never banks more than 0.048 Nm per sent tick; shorter-period ports should use the smaller of 9.6T and 0.048 Nm. At fixed 5 ms the tested filter coefficient is 0.393469. Reinitialize/track memory bumplessly on mode changes; maintain it across ordinary retargets. Freeze integral input on any previous output reshape (including the new FF guard) and on current envelope clamping; leak continues. Any retained legacy/assist path keeps its separately slewed `d_plain` and the current HoldTracking exclusion. GravityComp remains strictly model-only, including supported elevated holds; this proposal does not authorize position-only elevated support or weakened fuses.

Predicted prescribed-suite worst scores: **22.32 mrad slow track, 19.87% fast excess, 6.67 mrad stop, 0.04800 Nm FF step, zero clamps/faults, ≤0.384 mrad hold/repeat excursion**. Predicted slow true-velocity RMS about **0.044–0.047 rad/s**, versus baseline **0.055–0.060** on ripple plants. Torque jitter does not reliably improve. The wider uncertainty overshoot failure is an explicit deployment risk, not hidden by the nominal pass.

**Deciding bench plan, owner/Bonaparte only, through `bench-run`:**

1. Arm supported, physical E-stop within reach, clear workspace and known payload. Capture revision/config/URDF, read-only health/CAN counters, and every-tick trace. Acquire current physical reference and enable in the same owner process. Verify GravityComp/gravity residual gate first; any home model-fault trip ends the trial and routes to model correction, not more kp/ki.
2. Before any wider travel, compare baseline/candidate on identical supported near-home reciprocal motion at **0.05, 0.10, 0.20, 0.40 rad/s**, with **10%, then 25%, then 50%** of existing move velocity for stopping/reversal trials; do not raise caps. Include 20/50/100 mrad moves and five `0→0.496→0` repeats. Holds at elevated extrema stay supported and use GravityComp between motion tests. Any preflight refusal, latch, total clamp, >0.05 Nm FF step, unexpected sag/oscillation or >10 mrad stop ends the progression.
3. Collect candump plus every-tick host trace and retain **raw type-2 velocity before Davout substitution**, decoded MIT fields and receive timestamps. Fit torque once with raw velocity, once with host derivative, once with the smooth position velocity. Raw-velocity/torque kicks of approximately kd times the observed quantum, with a better residual fit than smooth velocity, would support the coarse-estimator hypothesis; the opposite result rejects it. Do not infer internal servo quantization from trace dq alone.
4. Score ADR criteria and, separately, the same 50 ms velocity-error RMS on matched speed/pose bands. The candidate must deliver **at least 15% lower matched slow velocity RMS** without worse stopping, no clamps and no new hold hunting; target is roughly 0.045 rather than 0.055 rad/s. If tracking improves but velocity RMS does not, reject the damping rationale and investigate the measured ripple/drive transfer, not more stiffness. Repeat spread and final-second drift must stay ≤10/5 mrad. Resolve changing inertia/estimator delay before full-speed fast qualification; replay's 27.36% uncertainty case makes that a required gate.

This is the sole recommended law. Do not cherry-pick physical parameters until the reduced-speed evidence confirms it.
