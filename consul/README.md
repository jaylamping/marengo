<p align="center">
  <img src="../docs/portraits/consul.jpg" alt="Napoleon in his study" width="420"/>
</p>

# consul

**Consul** — frontend.

Operator UI for Marengo (Vite + React + TypeScript): robot state, URDF visualization, and tuning. Consumes `assets/meshes/visual/` and talks to runtime over Chappe (binary protobuf, same schemas as [`proto/`](../proto/)).

**Status:** Vite + React 19 dashboard with optional live Chappe telemetry ([ADR 0008](../docs/decisions/0008-chappe-webtransport-transport.md)).

- **Robot-hosted (default on Pi):** open `https://marengo.local:8444` after `install-pi.sh` / `deploy-pi.sh --install`. Endpoints are derived from the page origin; WebTransport uses `:8443/chappe`.
- **Local dev:** copy `.env.example` → `.env.local` with `VITE_CHAPPE_*`, then `npm run dev` on `:5173`.

URDF visualization is still upcoming.

## Development

Requires [`buf`](../docs/dev-setup.md) and Node.js.

```bash
npm install
npm run gen:proto   # ../proto → src/gen/ (gitignored)
npm run build       # tsc --noEmit (CI parity)
```

`gen:proto` runs automatically before `dev` and `build`.

Generated TypeScript uses `@bufbuild/protobuf` — never edit `src/gen/` by hand.

See `DESIGN.md` and `TECH.md` for the guiding principles and current tech choices.
