# consul/src/pages/

## Responsibility

Thin page shells, imported statically by `routes/config.tsx` so the URL updates at once. Each shell lazy-loads its heavy overview body with `React.lazy` inside `DeferredLazyBody`. The shared chrome (`AppSidebar`, `SiteHeader`, `SceneBackground`) comes from `DashboardLayout` under `RootLayout`.

## Design

- **Pattern**: `export function XxxPage() { return <DeferredLazyBody><XxxOverview /></DeferredLazyBody>; }` with `XxxOverview` from `React.lazy`.
- `DashboardPage` renders `DashboardOverview` directly and takes an `active` flag so the cached Overview can pause off-route.
- `SubsystemsPage` is a legacy URL: it redirects to `/telemetry`.

## Pages

| Route | File | Component | Feature Overview |
|---|---|---|---|
| `/` | `dashboard.tsx` | `DashboardPage` | `DashboardOverview` — section cards and the CAN bus spectrum |
| `/simulation` | `simulation.tsx` | `SimulationPage` | `SimulationOverview` — simulation control and scenario browser |
| `/telemetry` | `telemetry.tsx` | `TelemetryPage` | `TelemetryOverview` — read-only live joint table |
| `/subsystems` | `subsystems.tsx` | `SubsystemsPage` | Redirects to `/telemetry` |
| `/hardware` | `hardware.tsx` | `HardwarePage` | `HardwareOverview` — master config, URDF import, Set Limits, Set Zero |
| `/logs` | `logs.tsx` | `LogsPage` | `LogsOverview` — structured log viewer with live stream |
| `/testing` | `testing.tsx` | `TestingPage` | `TestingOverview` — MIT command testing interface |

## Data flow per page

```
DashboardPage:     [AppProviders booted Chappe bridge] → robotStore / hostMetricsStore → DashboardOverview
SimulationPage:    Static scenario data from data/simulation.ts → SimulationOverview
TelemetryPage:     robotStore RobotState via useLiveInventory → TelemetryOverview (wire state only)
HardwarePage:      config snapshot + ActuatorLimitSnapshot + RobotState → HardwareOverview → gateway HTTP
LogsPage:          logBuffer singleton (ring buffer) + log-api.ts HTTP → LogsOverview
TestingPage:       testingStore (Zustand) → TestingOverview actions → chappe-client.ts HTTP POST
```

## Integration

| Page | Data Source | Telemetry Dependency |
|---|---|---|
| Dashboard | `robotStore`, `hostMetricsStore` | Full Chappe stream |
| Simulation | Static mock data | None |
| Telemetry | `useLiveInventory` over `robotStore` | RobotState, SafetyState and IMU from Chappe |
| Hardware | `/config/snapshot`, `actuatorStore`, `robotStore` | ActuatorLimitSnapshot and RobotState from Chappe |
| Logs | `logBuffer` + `log-api.ts` HTTP | Chappe LogEvent stream |
| Testing | `testingStore` → `chappe-client.ts` POST | Chappe command and state feedback |
