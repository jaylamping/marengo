# SDD / OpenSpec

Spec-driven development artifacts for Marengo. Default store mode is **openspec** (files only; no memory MCP).

## Layout

```
openspec/
├── config.yaml          # Project context, testing, phase rules
├── specs/               # Main specs (source of truth after archive)
└── changes/
    ├── archive/         # Completed changes (YYYY-MM-DD-{slug}/)
    └── {slug}/          # Active change artifacts + state.yaml
```

## Active change

- **log-api-error-states** — SDD onboarding cycle (Consul log-api error/loading states)

Run `/sdd-explore log-api-error-states` to start the explore phase.
