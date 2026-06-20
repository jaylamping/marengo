# consul/ — Operator UI (Vite + React + TS)

Separate npm workspace. Vite + React + TypeScript + shadcn/ui. Consumes Chappe wire types via protobuf codegen. **Never hand-edit `src/gen/`.**

## STRUCTURE

```
consul/
├── package.json          # npm workspace (separate from Cargo)
├── buf.gen.yaml          # protobuf → TS codegen config
├── vite.config.ts
├── DESIGN.md             # design system / UI conventions
├── TECH.md               # tech stack notes
└── src/
    ├── gen/              # GENERATED proto types — never hand-edit
    ├── components/       # React components (shadcn/ui based)
    ├── pages/            # Route-level pages
    ├── routes/           # Route definitions
    ├── hooks/            # React hooks
    ├── state/            # State management
    ├── lib/              # Utilities
    ├── data/             # Static data / constants
    ├── urdf/             # URDF viewer logic
    └── assets/
```

## WHERE TO LOOK

| Task | Location |
|------|----------|
| Regenerate proto types | `npm run gen:proto` (reads `buf.gen.yaml`, writes `src/gen/`) |
| Type-check | `npm run build` |
| Design system | `DESIGN.md`, `components.json` (shadcn config) |
| URDF viewer | `src/urdf/` |
| API / Chappe connection | `src/lib/`, `src/hooks/` |

## CONVENTIONS

- **Proto-first:** change `proto/` → `npm run gen:proto` → never touch `src/gen/`.
- npm lockfile must match CI (Linux Node 24 `npm ci`): `just consul-lock` / `just consul-ci`.
- `dev` script is a scaffold (`echo "Vite app scaffold TBD"`) — no running frontend dev server yet.

## ANTI-PATTERNS

- Hand-editing `src/gen/` — always regenerate.
- Adding JSON wire types alongside proto — protobuf only ([ADR 0001](../docs/decisions/0001-protobuf-wire-types.md)).
- Building on Windows without matching Linux lockfile — use `just consul-lock` in container.
