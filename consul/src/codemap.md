# consul/src/

## Responsibility
React application source — routing, state, Chappe client, dashboard components, and generated proto types.

## Design
| Area | Path | Role |
|------|------|------|
| Routes | `routes/` | `appRouter`, route config |
| Pages | `pages/` | Top-level page components |
| State | `state/` | Zustand stores (testing, host metrics, telemetry) |
| Client | `lib/chappe-client.ts` | WebTransport/HTTP stream, enable POST, health check |
| Components | `components/dashboard/` | Operator panels (testing, URDF, logs, metrics) |
| Data | `data/` | Static robot inventory metadata |
| Assets | `assets/urdf/` | Client-side URDF helpers for preview |
| Generated | `gen/` | Buf protobuf TypeScript output |

## Flow
App mount → `connectChappeStream(handlers)` → envelope dispatch → React re-render from stores
Enable button → `postEnableCommand(true)` → gateway → Chappe EnableRequest → marengo-pi

## Integration
- Gateway endpoints: `/health`, WebTransport subscribe, enable/disable API
- See nested maps: [lib/codemap.md](lib/codemap.md), [state/codemap.md](state/codemap.md), [components/dashboard/codemap.md](components/dashboard/codemap.md)
