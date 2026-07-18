# scripts/

## Responsibility
**Build, deploy, and bench automation** shell/Python scripts for cross-compilation, Pi rsync, CAN setup, CI checks, and systemd installation.

## Design
| Script | Role |
|--------|------|
| `deploy-pi.sh` | Cross-build aarch64, rsync staging to Pi, optional Consul npm build |
| `deploy-lib.sh` | Shared deploy helpers, host resolution, progress env |
| `pi-remote.sh` | SSH wrapper for remote Pi commands |
| `check.sh` | CI: fmt, clippy, test, deny |
| `install-pi.sh` | Install staging tree to `/opt/marengo` on Pi |
| `vcan-setup.sh` | Virtual CAN for dev without hardware |
| `daily-audit/` | Automated audit scripts |

## Flow
Developer/MCP deploy path:
1. `cargo build --target aarch64-unknown-linux-gnu` (or cross via deploy-pi.sh)
2. `deploy-pi.sh <pi-host>` → rsync bins + config + Consul dist
3. `install-pi.sh` on Pi → `/opt/marengo`, systemd reload
4. MCP `pi_sync_main` orchestrates full sync + health poll

## Integration
- **Targets**: Pi bench host (`MARENGO_PI_HOST`), `/opt/marengo` install root
- **Used by**: developers, MCP marengo-pi tools, GitHub Actions CI
