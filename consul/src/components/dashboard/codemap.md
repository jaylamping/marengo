# consul/src/components/dashboard/

## Responsibility
**Operator dashboard panels** — realtime robot control, visualization, and diagnostics UI.

## Design
| Panel | Role |
|-------|------|
| `testing/` | Enable/disable, hold-at, PID sliders, gravity-on |
| `urdf-preview/` | 3D URDF visualization of current joint states |
| `simulation/` | Simulation mode controls |
| `logs/` | Structured log tail from gateway |
| `metrics/` | Host metric charts |
| `inventory/` | Robot hardware inventory cells |
| `overview/` | Summary cards and subsystem status |
| `layout/`, `sidebar/`, `site-header/` | Chrome and navigation |

## Flow
Store subscription → panel render → user action → API POST or Chappe command publish via gateway

## Integration
- Depends on `state/`, `lib/chappe-client.ts`, shadcn `components/ui/`
