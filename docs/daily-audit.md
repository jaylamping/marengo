# Daily audit

Hybrid daily review for [jaylamping/marengo](https://github.com/jaylamping/marengo):

1. **GitHub Actions** (06:00 UTC) — deterministic checks → JSON/Markdown report → GitHub issue
2. **Cursor Automation** (07:00 UTC) — qualitative industry review (see below)

Rubric: [daily-audit-rubric.md](daily-audit-rubric.md)

## Local dry-run

```bash
just daily-audit
# or
./scripts/daily-audit/run.sh
```

Reports: `var/log/daily-audit/YYYY-MM-DD/report.json` and `report.md`

## Deterministic checks

Implemented in [scripts/daily-audit/](../scripts/daily-audit/):

- `unwrap`/`expect` in changed `crates/` files
- Hand-edits to `consul/src/gen/`
- Davout bypass patterns in motor paths
- Large diffs in safety-critical crates
- ADR staleness vs changed paths
- Hardware/config coupling
- `cargo audit` (when available)
- Open PR file overlap for findings

Only **warn** and **critical** findings open/update a GitHub issue (`daily-audit` label).

## Research appendix (headless)

After deterministic audit, optional industry context:

```bash
cd tools/marengo-research-mcp
uv run python -m marengo_research_mcp.cli audit-research \
  --topics-file ../../var/log/daily-audit/YYYY-MM-DD/topics.json \
  -o ../../var/log/daily-audit/YYYY-MM-DD/research.md
```

## Cursor Automation (qualitative layer)

Create in Cursor Automations UI (not stored in repo):

| Field | Value |
|-------|-------|
| Name | Marengo daily standards review |
| Trigger | Cron `0 7 * * *` (07:00 UTC) |
| Repo | `jaylamping/marengo` / `main` |

### Agent instructions

1. Read [docs/daily-audit-rubric.md](daily-audit-rubric.md)
2. Download or re-run latest deterministic report from GitHub Actions artifact / `just daily-audit`
3. List commits and open PRs from last 24 hours
4. For each changed subsystem, call **`research_humanoid`** (local MCP) or run `audit-research` CLI
5. Merge qualitative findings; cite project rules and external URLs
6. Upsert GitHub issue with label `daily-audit`
7. Comment on open PRs whose files overlap **warn/critical** findings
8. If clean (no warn+): close open `daily-audit` issue with "All clear on YYYY-MM-DD"

### MCP note

Project MCP (`marengo-research`) may not appear in Cursor Automations dashboard. Options:

- Run qualitative review from **local agent** with MCP enabled
- Use **`audit-research` CLI** in GitHub Actions (wired in workflow)
- Cloud agents: **`./scripts/pi-remote.sh`** over Tailscale for Pi logs (see [cloud-pi-tailscale.md](cloud-pi-tailscale.md))

### WorkflowData draft (for Automations editor)

```yaml
name: Marengo daily standards review
description: Qualitative humanoid/industry alignment check after deterministic daily audit
workflow:
  triggers:
    - cron:
        cron: "0 7 * * *"
  actions: []
  prompts:
    - |
      You audit jaylamping/marengo daily. Read docs/daily-audit-rubric.md.
      Use the latest deterministic daily-audit report. Review last 24h commits and open PRs.
      Call research_humanoid for subsystem topics (motors, control, sim, hardware).
      File or update a GitHub issue labeled daily-audit for warn/critical findings with evidence.
      Comment on open PRs that touch flagged files. Close the issue if all clear.
  gitConfig:
    repo: jaylamping/marengo
    branch: main
```

Adjust repo/branch in the Automations editor if needed.
