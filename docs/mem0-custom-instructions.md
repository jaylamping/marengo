# mem0 custom instructions (paste into dashboard)

Use case: Marengo humanoid robotics project — Rust control stack, Pi bench, SolidWorks CAD, spec-driven development.

```
Prioritize durable engineering facts: motor/CAN bring-up lessons, CAD mate decisions, feasibility verdicts, SDD phase summaries, and research distillations.

Namespaces:
- sdd/{change}/{phase} — SDD artifacts
- feasibility/{change}/brief — go/no-go briefs
- decision/{area}/{slug} — cross-cutting engineering decisions
- hardware/{subsystem}/{slug} — mechanical/electrical facts
- cad/{assembly}/{slug} — SolidWorks knowledge
- pi/{subsystem}/{slug} — Pi deploy and bench ops
- control/{subsystem}/{slug} — motors, safety, kinematics
- software/{crate}/{slug} — Rust, Consul, proto, CI
- research/{domain}/ — background research
- expert/{domain}/ — curated heuristics
- maintenance/session-handoff/{project} — session resume
- maintenance/skill-registry — skill index
- maintenance/prune/{date} — prune audits

Never store API keys, passwords, private keys, or bench tokens. Reject secrets and raw logs.

For pi/ and cad/ memories, include observed_at, source_ref, and valid_until when state can go stale.

Marengo safety is authoritative: motor commands via Davout, no unwrap in library code, Pi MCP for bench work, CAD worktree safety before binary restores.

Link mental model to AGENTS.md gates: just check, proto-first, hardware safety.
```

Test message: "Shoulder pitch hold failed with CAN timeout on bench — we fixed stale config sync via pi_sync_main."
