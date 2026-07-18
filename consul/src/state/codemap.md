# consul/src/state/

## Responsibility
**Zustand stores** for live telemetry, testing panel state, and host metrics.

## Design
- `testingStore.ts`: hold-at position, PID gains, enable state for bench testing panel
- `hostMetricsStore.ts`: CPU/mem/disk from `host/metrics/pi` topic
- Stores updated by `dispatchEnvelope` handlers from chappe-client

## Flow
Envelope `robot/state` → parse RobotState → `testingStore.setJointStates(...)`
Envelope `host/metrics/pi` → parse HostMetrics → `hostMetricsStore.update(...)`

## Integration
- Consumed by `components/dashboard/testing/`, metrics cards, overview panels
