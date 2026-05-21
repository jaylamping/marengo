<p align="center">
  <img src="../docs/portraits/consul.jpg" alt="Napoleon in his study" width="420"/>
</p>

# consul

**Consul** — frontend.

Operator UI for Marengo (Vite + React + TypeScript): robot state, URDF visualization, and tuning. Consumes `assets/meshes/visual/` and talks to runtime over Chappe (binary protobuf, same schemas as [`proto/`](../proto/)).

**Status:** proto codegen + TypeScript check only; Vite app is planned ([docs/roadmap.md](../docs/roadmap.md) M7). Do not add UI features until Chappe has a multi-process transport ADR.

## Development

Requires [`buf`](../docs/dev-setup.md) and Node.js.

```bash
npm install
npm run gen:proto   # ../proto → src/gen/ (gitignored)
npm run build       # tsc --noEmit (CI parity)
```

`gen:proto` runs automatically before `build`. `npm run dev` is a placeholder until the Vite scaffold lands. Generated TypeScript uses `@bufbuild/protobuf` — never edit `src/gen/` by hand.
