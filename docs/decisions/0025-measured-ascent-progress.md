# ADR 0025: measured encoder progress owns the ascent watchdog

Status: accepted for software implementation, September 30, 2026.

## Context

CS24's no-progress budget uses the sign of filtered velocity. Its positive EMA
tail never becomes exactly zero after motion stops. The same filter also gates
the watchdog lifetime, allowing repeated velocity spikes or bounded encoder
jitter to discard the budget. Integer `1000 / hz` adds no time above 1,000 Hz.
PositionHold computes configured inactive peers before output is scoped, so an
unresolved disabled peer can fault a healthy selected position owner.

## Decision

Keep existing planner recovery, lead bounds, resync, friction and gains. A
separate geometric watchdog watches only actually commanded joints with a
non-home target ahead in the positive joint direction, beyond the existing return-settle band of
measured q. It retains a credited encoder high-water mark. A qualifying new
level resets the existing 2,000 ms budget; unchanged measurements, retreat,
revisiting old peaks, velocity changes and planner resync do not. Subthreshold
movement accumulates against the last credited level. Settle, true retarget,
leaving Position, stop/fault cleanup and the existing Wave ownership exemption
end the episode. An identical-target retarget preserves it.

Davout supplies a read-only joint-space threshold from its frozen installed
receive conversion. Robstride owns the actual decoder's signed-field center
and model scale; Berthier consumes the threshold without interpreting CAN or
mutable motor configuration. Intent cleanup preserves the installed profile.
Whole Supervisor/model replacement and coordinated configuration installation
remain CS15/CS21; this metadata is no reference grant or replacement-owner fix.

For exact f32 scale S, positive installed gear g and signed-field center N,
the nominal adjacent step is s=S/(N*g). Let m=16*EPSILON_f32*S/g and use
epsilon=s/2+m. The actual decoder's three f32 operations plus Davout's f64
division/f32 cast have endpoint error below 7*u32*S/g; m=32*u32*S/g bounds it.
Therefore adjacent converted levels are at least s-2m apart. Current N=32767
gives m/s approximately 0.062498, so epsilon is strictly below that separation.
Validate finite positive quantities, epsilon < s-2m, normal nonzero converted
levels (`s-2m >= MIN_POSITIVE_f32`) and a finite full decoded range
(`S/g + 2*s <= MAX_f32`). Unsupported numeric profiles fail explicitly.
This is a software mapping bound, not measured physical encoder noise.

The standalone law constructor retains ideal continuous inputs with only a
f64 roundoff floor. Every real ControlLoop installs Davout's validated grid.
Both paths use the same algorithm; no test-only safety policy or public permit
setter is introduced. Existing HoldWorld/HoldJointParams literal shapes stay
usable by the independently frozen original-interface probes.

Accumulate a Duration from the same nominal dt used by the planner. Preserve
explicit zero-dt pure composition; reject negative, nonfinite, overflowing or
positive durations that round to zero before mutating state. Actual loop
construction rejects a zero-rounded period before Supervisor construction.
The default 200 Hz/5 ms period and 2-second fuse remain unchanged. This does
not establish physical scheduling, acquisition timing or 1,250 Hz capability.

## Verification and limits

Require unchanged-source behavioral failures and byte-identical repaired
replays for motion-then-stop, jitter/reversal/held velocity, submillisecond
timing, inactive-peer scope and zero-rounded construction. Qualify actual
ControlLoop fault propagation, all installed stop actions and persistent
refusal after healthy feedback. A literal raw one-count staircase must survive
beyond two seconds without planner echo; a held count must exhaust its budget.
Validate rounding at independent raw endpoints and non-unit conversions;
new-interface numeric/time rejection tests are conformance, not fake original
reds. Keep archive-only wrappers outside the default suite and retain useful
existing recovery, lead/cap, retarget and Wave tests.

Physical sensor noise/plant/stop/support acceptance, current reference
acquisition, drive limit/timeout qualification and corrected-model
commissioning remain external. Do not operate the robot, raise limits, change
Wave sign-off or infer physical readiness from software checks.
