# Ascent-stall recovery — arena synthesis

## Base

**candidate-5** — explicit `AscentRecovery { Idle | Active { stalled_ms } }`; planner advances instead of freezing; existing lead clamp + stuck-lead resync provide bounded authority; fuse sticky across resync.

Parent initially preferred candidate-1 (freeze + lead-follow escalator). Cross-judge preferred candidate-5 for cleaner lifecycle and fewer load-bearing special cases (no resync-suppression footgun). Parent agreed: same authority class (reach `max_lead` P-torque), easier to extend.

## Grafts

| From | Kept | Why |
|------|------|-----|
| candidate-1 | `PlannerEvent::AscentBreakaway`, `ascent_stall_ms` on hold diag + 1 Hz log, motion-release test | Observability + mutation-resistant “progress clears fuse / planner resumes” lock without changing C5’s law |
| candidate-4 | fuse-survives-resync timing assertion (`fault_tick − fuse_armed ≤ 410`) | Locks sticky fuse against silent resync resets; no CSV schema change |

## Rejected

| Candidate / idea | Why |
|------------------|-----|
| candidate-1 as base | Lead-follow rewrite + resync suppression while frozen is load-bearing; removing the gate fail-opens |
| candidate-2 | Snap-resync at 0.03; test asserts `max_lead < 0.04` — never delivers breakaway torque |
| candidate-3 | Sticky fuse + onset, still freezes at ~0.03 — underpowered for the friction knee |
| candidate-4 as base / remove freeze alone | Same family as C5 but larger surface (CSV column); C5’s named state is the cleaner boundary |
| candidate-4 CSV `ascent_stall_ms` column | Contract churn; diag + 1 Hz log is enough |
| Unify freeze/resync at 0.03 without sticky fuse | Fail-open retry loop (C1 mutation) |
| Re-enable `approach_stuck_mit_pull` | Documented open-loop grind regression |

## Dropouts

None. All five candidates produced artifacts + `RATIONALE.md`.

## Verification

See Phase F log after `cargo test -p berthier`.
