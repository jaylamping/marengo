# Ultrawork Notepad — Continue gravity-comp bench T4–T10 after stuck-pull fix
Started: 2026-06-21T00:00:00-05:00

## Plan (exhaustive, atomic)
TBD after plan-agent + state check.

## Scenarios (the contract)
TBD — must cover T4 0.1 rad/s positive, T4 0.5/1.0 both directions, config restore, T5–T10.

## Now (single step in progress)
Gather state and handoff context.

## Todo (remaining, ordered)
1. Confirm local working tree state vs handoff.
2. Confirm Pi working tree is dirty and needs cleaning.
3. Plan agent: produce parallel task graph for clean→deploy→T4–T10.
4. Clean Pi tree with `pi_clean_tree` (stash or reset-hard per user preference).
5. Deploy patched `marengo-pi` to Pi.
6. Run T4 friction sweep 0.1 rad/s positive to 1.484 rad.
7. Capture trace + analyze; verify pass/fail.
8. Repeat T4 at 0.5 and 1.0 rad/s, both directions.
9. Restore default trajectory velocity limit and sync config.
10. Continue T5–T10 per test-suite doc.

## Findings (non-obvious facts with file:line refs)
- Handoff: `crates/berthier/src/loop.rs` stuck-pull now uses `max(POSITION_DESCENT_STUCK_LEAD_RAD, effective_max_lead)`.
- `pi_clean_tree` MCP tool is compiled and tool list in this session already includes it.
- Prior `pi_build` succeeded but `install-pi.sh` sudo password blocked /opt install.

## Learnings (patterns / pitfalls for next turn)
- Weighted profile motion calls need both `confirm: true` and `confirm_weighted_motion: true`.
- Use `pi_sync_main` with `strategy: "pi_native"` because cross compiler absent on Windows host.
