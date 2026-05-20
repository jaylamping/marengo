<p align="center">
  <img src="../docs/portraits/consul.jpg" alt="Napoleon in his study" width="420"/>
</p>

# consul

**Consul** — frontend.

Operator UI for Marengo (Vite + React + TypeScript): robot state, URDF visualization, and tuning. Consumes `assets/meshes/visual/` and talks to runtime over Chappe (binary protobuf, same schemas as [`proto/`](../proto/)).

## Development

Requires [`buf`](../docs/dev-setup.md) and Node.js.

```bash
npm install
npm run gen:proto   # ../proto → src/gen/ (gitignored)
npm run dev
```

`gen:proto` runs automatically before `dev` and `build`. Generated TypeScript uses `@bufbuild/protobuf` — never edit `src/gen/` by hand.
