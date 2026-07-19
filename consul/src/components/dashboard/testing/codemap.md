# consul/src/components/dashboard/testing/

## Responsibility

Operator Testing tab: manual hold-at/PID, compound presets (Wave), and teach-record
(GravityComp capture → landmarks → Wave overlay).

## Design

- `compound-test-panel.tsx` — shipped presets + optional taught overlay; Wave keeps
  `nativeWave` until Teach Apply.
- `teach-record-panel.tsx` — separate UI; Record gated on gravity-armed + ACTIVE;
  no POSITION posts during Record.
- Cadence/dwell only in teach transit (not Berthier speed fantasy).
- Soft-invalidate: set-zero bumps calibration epoch (`I set-zero'd`); overlay kept;
  Wave blocked until Acknowledge & keep or Reset. Ordinary Home does not bump.

## Flow

Chappe RobotState → (unthrottled) teach-sample-bus → teachStore buffer → landmarks
→ overlay in localStorage → compound runner resolves effective preset.

## Integration

`@/lib/teach-record`, `@/lib/teach-transit`, `@/state/teachStore`, gateway MIT batch
for compound playback only.
