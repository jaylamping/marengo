# consul/src/components/dashboard/testing/

## Responsibility

Operator Testing tab: manual hold-at/PID and compound presets with per-preset
**Manual Movement** (GravityComp capture → landmarks → Wave overlay) and
**Auto Learn** (Cursor BFF → stage-capped teach draft → same overlay path).

## Design

- `compound-test-panel.tsx` — shipped presets + optional taught overlay; stack is
  Joint Trims → Auto Learn → Manual Movement → Playback. Wave keeps `nativeWave`
  until Teach Apply.
- `manual-movement-panel.tsx` — GravityComp teach-record; gated on gravity-armed +
  ACTIVE; no POSITION posts during Record.
- `auto-learn-panel.tsx` — crawl→walk→run drafts via local BFF; own review strip;
  Dry Run soft-gate + stage speed ceiling on playback.
- Cadence/dwell only in teach transit (not Berthier speed fantasy).
- Soft-invalidate: set-zero bumps calibration epoch (`I set-zero'd`); overlay kept;
  Wave blocked until Acknowledge & keep or Reset. Ordinary Home does not bump.

## Flow

Chappe RobotState → (unthrottled) teach-sample-bus → teachStore buffer → landmarks
→ overlay in localStorage → compound runner resolves effective preset.

Auto Learn: Consul snapshot → `tools/compound-auto-learn` `Agent.prompt` → asserts
→ draft → Apply → same teach overlay.

## Integration

`@/lib/teach-record`, `@/lib/teach-transit`, `@/state/teachStore`,
`@/state/autoLearnStore`, `@marengo/compound-auto-learn`, gateway MIT batch for
compound playback only.
