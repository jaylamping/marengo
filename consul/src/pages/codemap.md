# consul/src/pages/

## Responsibility

Thin page components that serve as react-router `lazy()` entry points. Each page composes the shared `DashboardLayout` shell with its feature-specific overview component. Pages are the only modules loaded lazily — all code splitting boundaries align with page boundaries.

## Design

- **Pattern**: Every page follows `export function XxxPage() { return <DashboardLayout><XxxOverview /></DashboardLayout>; }`.
- `DashboardLayout` provides the persistent chrome: `AppSidebar`, `SiteHeader`, `SceneBackground`, scroll container.
- Feature overview components are imported statically (they live in the same chunk as the page).
- `SubsystemsPage` is the only page that prepares data before rendering: it calls `useLiveInventory(robotInventory)` to overlay live Chappe joint positions onto the static inventory, then passes the enriched array to `SubsystemsOverview`.

## Pages

| Route | File | Component | Feature Overview |
|---|---|---|---|
| `/` | `dashboard.tsx` | `DashboardPage` | `DashboardOverview` — host cards, charts, URDF preview |
| `/simulation` | `simulation.tsx` | `SimulationPage` | `SimulationOverview` — simulation control and scenario browser |
| `/subsystems` | `subsystems.tsx` | `SubsystemsPage` | `SubsystemsOverview` — inventory table with live joint data |
| `/logs` | `logs.tsx` | `LogsPage` | `LogsOverview` — structured log viewer with live stream |
| `/memory` | `memory.tsx` | `MemoryPage` | `MemoryOverview` — mem0 Memory Observatory |
| `/testing` | `testing.tsx` | `TestingPage` | `TestingOverview` — MIT command testing interface |

## Data flow per page

```
DashboardPage:     [AppProviders booted Chappe bridge] → robotStore / hostMetricsStore → DashboardOverview
SimulationPage:    Static scenario data from data/simulation.ts → SimulationOverview
SubsystemsPage:    robotInventory (static) + useLiveInventory (live overlay) → SubsystemsOverview
LogsPage:          logBuffer singleton (ring buffer) + log-api.ts HTTP → LogsOverview
MemoryPage:        mem0-api.ts HTTP (React Query) → MemoryOverview
TestingPage:       testingStore (Zustand) → TestingOverview actions → chappe-client.ts HTTP POST
```

## Integration

| Page | Data Source | Telemetry Dependency |
|---|---|---|
| Dashboard | `robotStore`, `hostMetricsStore` | Full Chappe stream |
| Simulation | Static mock data | None |
| Subsystems | `data/robot-inventory.ts` + `useLiveInventory` | RobotState from Chappe (optional enrichment) |
| Logs | `logBuffer` + `log-api.ts` HTTP | Chappe LogEvent stream |
| Memory | `mem0-api.ts` HTTP (React Query) | None (independent service) |
| Testing | `testingStore` → `chappe-client.ts` POST | Chappe command and state feedback |
