<p align="center">
  <img src="../docs/portraits/consul.jpg" alt="Napoleon in his study" width="420"/>
</p>

# consul

Consul is the operator frontend for Marengo (Vite + React + TypeScript): robot state, URDF visualization, tuning. Loads `assets/meshes/visual/` and talks to the runtime over Chappe (binary protobuf, same schemas as [`proto/`](../proto/)).

Vite + React 19 dashboard with optional live Chappe telemetry ([ADR 0008](../docs/decisions/0008-chappe-webtransport-transport.md)).

- Robot-hosted (default on Pi): open `https://marengo.local:8444` after `install-pi.sh` / `deploy-pi.sh --install`. Endpoints derive from the page origin; WebTransport uses `:8443/chappe`. Deploy scrubs `VITE_CHAPPE_*` (see `consul/.env.production` + `scripts/check-consul-dist.sh`).
- Local dev: copy `.env.example` → `.env.local` with `VITE_CHAPPE_*`, then `npm run dev` on `:5173`. **`.env.local` is dev-only** — Pi deploy must not bake those URLs.

### Operator triage

| Badge | Meaning |
|-------|---------|
| LIVE / CONNECTING | Chappe endpoints resolved; connecting or streaming |
| **CHAPPE ERR** | Hover for HTTP/WT URLs. `127.0.0.1` → baked dev env; redeploy. Derived URLs + gateway hint → check `marengo-gateway` |
| **WIREFRAME** | HTTP page or no endpoints (mock UI) |

After deploy-pipeline fixes merge, **`pi_sync_main`** redeploys a clean bundle and updates `/opt/marengo/.deploy-rev`. Details: [chappe-consul-ingestion.md](../docs/chappe-consul-ingestion.md), [ADR 0008](../docs/decisions/0008-chappe-webtransport-transport.md).

URDF visualization is still upcoming.

## Development

Requires [`buf`](../docs/dev-setup.md) and Node.js.

```bash
npm install
npm run gen:proto   # ../proto → src/gen/ (gitignored)
npm run build       # tsc --noEmit (CI parity)
```

`gen:proto` runs automatically before `dev` and `build`.

Generated TypeScript uses `@bufbuild/protobuf`. Never edit `src/gen/` by hand.

`DESIGN.md` and `TECH.md` cover principles and current tech choices.
