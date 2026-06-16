# mem0 custom instructions (paste into dashboard)

Use case: Marengo humanoid robotics project — Rust control stack, Pi bench, SolidWorks CAD, spec-driven development.

```
Prioritize durable engineering facts: motor/CAN bring-up lessons, CAD mate decisions, feasibility verdicts, SDD phase summaries, and research distillations.

Namespaces:
- sdd/{change}/{phase} — SDD artifacts
- feasibility/{change}/brief — go/no-go briefs
- research/{domain}/ — background research
- expert/{domain}/ — curated heuristics
- maintenance/prune/{date} — prune audits

Never store API keys, passwords, private keys, or bench tokens. Reject secrets.

Marengo safety is authoritative: motor commands via Davout, no unwrap in library code, Pi MCP for bench work, CAD worktree safety before binary restores.

Link mental model to AGENTS.md gates: just check, proto-first, hardware safety.
```

Test message: "Shoulder pitch hold failed with CAN timeout on bench — we fixed stale config sync via pi_sync_main."
