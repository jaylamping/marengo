# consul/

## Responsibility
**Operator web UI** (Consul) — Vite + React + TypeScript dashboard for robot telemetry, enable/disable, testing panels, URDF preview, logs, and host metrics. Separate npm workspace from Rust Armée.

## Design
- **SPA** with React Router (`createBrowserRouter`)
- **State management**: Zustand stores in `src/state/`
- **Telemetry client**: `chappe-client.ts` — WebTransport primary, HTTP stream fallback
- **Component library**: shadcn/ui primitives under `components/ui/`
- **Dashboard panels**: testing (hold-at, PID sliders), simulation, URDF preview, inventory, logs, metrics
- Buf-generated protobuf types in `src/gen/`

## Flow
1. Browser loads Consul static assets (served by marengo-gateway or Vite dev server)
2. `connectChappeStream` → WebTransport to gateway `:8443` or HTTP fallback
3. `dispatchEnvelope` routes topics to Zustand store updates
4. User actions → `postEnableCommand`, testing commands → gateway HTTP POST → Chappe → marengo-pi

## Integration
- **Backend**: marengo-gateway (HTTP + WebTransport + Chappe IPC)
- **Proto**: shared with `proto/marengo/v1/marengo.proto`
- **Deploy**: built by `deploy-pi.sh`, served from gateway `--web-root`

**Detailed map**: [src/codemap.md](src/codemap.md)
