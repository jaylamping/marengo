# Repository Atlas: Marengo

## Project Responsibility
One repository for a personal humanoid robot: CAD, wiring, URDF, and the Rust runtime (Armée workspace). SolidWorks and harness docs define joints, frames, and limits; control, safety, planning, and the operator UI read that same definition. When the robot changes in CAD, software changes with it.

## System Entry Points
| Entry | Role |
|-------|------|
| `Cargo.toml` | Armée workspace root — 16 crates + 9 bins, `#![forbid(unsafe_code)]` |
| `bins/marengo-pi/src/main.rs` | Pi control loop, CAN, Chappe telemetry, operator REPL |
| `bins/marengo-gateway/src/main.rs` | HTTP/WebTransport gateway for Consul |
| `bins/motor-repl/src/main.rs` | Bench motor CLI (bring-up, set-zero, jog) |
| `consul/` | Vite+React operator dashboard (npm workspace) |
| `config/` | robot.yaml, motors.yaml, control.yaml + bringup profiles |
| `proto/marengo/v1/marengo.proto` | Wire schemas for Chappe (proto-first, ADR 0001) |
| `assets/urdf/` | URDF exported from CAD — kinematic source of truth |
| `justfile` | Developer task runner (check, deploy, sim) |

## Architecture Overview

```
Consul (React) ←HTTP/WT→ marengo-gateway ←IPC→ Chappe ← marengo-pi
                                                      ↓
                                              Berthier (control loop)
                                                      ↓
                                              Davout (safety gateway)
                                                      ↓
                                              robstride (CAN driver)
                                                      ↓
                                              Robstride motors
```

**Codenames**: Berthier=control, Davout=safety, Chappe=message bus, Talleyrand=planning, Fouché=vision, Consul=web UI.

## Directory Map (Aggregated)

| Directory | Responsibility Summary | Detailed Map |
|-----------|------------------------|--------------|
| `crates/` | Armée libraries: kinematics, dynamics, control loop, safety gateway, CAN driver, config, Chappe bus | [View Map](crates/codemap.md) |
| `bins/` | Thin runtimes: Pi control, gateway, motor REPL, Jetson, log CLI, probes | [View Map](bins/codemap.md) |
| `config/` | Declarative YAML: robot, motors, control, homing + bringup profiles | [View Map](config/codemap.md) |
| `proto/` | Protobuf wire types for Chappe (Rust + TS codegen) | [View Map](proto/codemap.md) |
| `consul/` | Operator web UI — telemetry, enable, testing panels, URDF preview | [View Map](consul/codemap.md) |
| `scripts/` | Cross-build, deploy-pi, check.sh, Pi install, vcan setup | [View Map](scripts/codemap.md) |

### Key Crates (quick reference)

| Crate | Role | Map |
|-------|------|-----|
| `berthier` | Realtime outer control loop | [crates/berthier/codemap.md](crates/berthier/codemap.md) |
| `davout` | Safety gateway (sole motor path) | [crates/davout/codemap.md](crates/davout/codemap.md) |
| `robstride` | Robstride CAN driver (MIT Mode 0) | [crates/robstride/codemap.md](crates/robstride/codemap.md) |
| `chappe` | Inter-process pub/sub bus | [crates/chappe/codemap.md](crates/chappe/codemap.md) |
| `armee-dynamics` | Gravity compensation τ_g(q) | [crates/armee-dynamics/codemap.md](crates/armee-dynamics/codemap.md) |
| `armee-kinematics` | URDF limits and envelope | [crates/armee-kinematics/codemap.md](crates/armee-kinematics/codemap.md) |
| `marengo-config` | YAML config loaders | [crates/marengo-config/codemap.md](crates/marengo-config/codemap.md) |

### Key Bins (quick reference)

| Binary | Role | Map |
|--------|------|-----|
| `marengo-pi` | Pi runtime | [bins/marengo-pi/codemap.md](bins/marengo-pi/codemap.md) |
| `marengo-gateway` | Operator gateway | [bins/marengo-gateway/codemap.md](bins/marengo-gateway/codemap.md) |
| `motor-repl` | Bench motor CLI | [bins/motor-repl/codemap.md](bins/motor-repl/codemap.md) |

## Data Flow (control tick)
1. Davout drains CAN feedback → joint positions in joint space
2. Berthier computes τ_g via armee-dynamics, adds friction/impedance terms
3. MIT command batch → Davout filters (limits, caps, danger zones) → robstride CAN encode
4. marengo-pi publishes RobotState on Chappe → gateway → Consul dashboard

## Related Documentation
- Human overview: [README.md](README.md)
- Agent index: [AGENTS.md](AGENTS.md)
- Architecture: [docs/architecture.md](docs/architecture.md)
- Safety: [docs/safety.md](docs/safety.md)
