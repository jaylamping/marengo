# scripts/ — Build, deploy, CI, bench tooling

Shell + Python scripts. No Rust logic here — these orchestrate the workspace, deploy to Pi/Jetson, and provide bench diagnostics.

## STRUCTURE

```
scripts/
├── check.sh                  # CI-parity: build + fmt + clippy + test + deny (host-native)
├── check-sim.sh              # MuJoCo sim checks
├── check-println-crates.sh   # Lint: no println! in crates
├── check-consul-dist.sh      # Consul build verification
├── bootstrap.sh              # Dev container first-run setup
├── can-up.sh                 # Bring up CAN interfaces
├── vcan-up.sh                # Virtual CAN bus
├── export-urdf.sh            # CAD → URDF export
├── validate-urdf.sh          # URDF validation
├── urdf-to-mjcf.sh           # URDF → MuJoCo conversion
├── proto-checksum.sh         # Proto codegen drift check
├── deploy-pi.sh              # Cross-build + deploy to Pi (macOS native)
├── deploy-pi-docker.sh       # Cross-build via Docker (Windows)
├── deploy-pi-docker.ps1      # PowerShell wrapper
├── deploy-jetson.sh          # Jetson deploy
├── install-pi.sh             # Install staging tree → /opt/marengo on Pi
├── pi-remote.sh              # Cloud fallback: SSH to Pi for health/logs/deploy
├── pi-native-build.sh        # Native cargo build on Pi
├── pi-bno085-shtp-init.py    # BNO085 SHTP init verification
├── pi-i2c-plain-read.py      # Plain I2C read test (not smbus)
├── setup-cloud.sh            # Cloud VM: protoc + cargo-deny + advisory-db
├── setup-cloud-pi.sh         # Cloud VM: Tailscale + SSH to Pi
├── setup-mac-pi-cross.sh     # macOS aarch64 cross-compile setup
├── setup-wsl-pi-cross.sh     # WSL2 cross-compile setup
├── homing-preflight.sh       # Pre-enable homing checks
├── log-inventory.sh          # Bench log inventory
├── bench-log-archive.sh      # Archive bench sessions
├── bench-log-prune.sh        # Prune old bench logs
├── bench-set-weighted-mass.sh # Set weighted-arm mass for bench tests
├── measure-can-mit-rate.sh   # CAN MIT frame rate measurement
├── analyze-position-trace.py # Parse position traces
├── profile-pi-loop.sh        # Pi control loop profiling
├── ci-vcan-host-setup.sh     # CI: vCAN host setup
├── test-compose-ssh.sh       # Docker compose SSH test
├── daily-audit/              # Daily audit pipeline (run.sh)
├── fixtures/                 # Test fixtures
└── systemd/                  # systemd unit files (marengo-can, marengo-pi, etc.)
```

## WHERE TO LOOK

| Task | Command |
|------|---------|
| CI-parity check | `./scripts/check.sh` (or `just check-native`) |
| Deploy to Pi (macOS) | `./scripts/deploy-pi.sh --install` |
| Deploy to Pi (Docker/Windows) | `./scripts/deploy-pi-docker.sh` |
| Pi health/logs (cloud) | `./scripts/pi-remote.sh health` / `logs-tail` / `logs-grep` |
| CAN diagnostics | `./scripts/measure-can-mit-rate.sh`, `marengo-log-cli candump summary` |
| URDF export | `./scripts/export-urdf.sh` |
| Sim conversion | `./scripts/urdf-to-mjcf.sh` |

## CONVENTIONS

- `check.sh` is the CI-parity entry point — `just check-native` wraps it.
- Pi scripts assume Tailscale SSH in cloud, `marengo.local` mDNS on LAN.
- `pi-remote.sh` is the cloud fallback when marengo-pi MCP is unavailable.
- systemd units in `systemd/` are the production runtime definitions.
- Python scripts use plain stdlib — no venv required for diagnostics.

## ANTI-PATTERNS

- Asking user to paste Pi logs when `pi-remote.sh` can fetch them.
- Running `cargo audit` that auto-updates the pinned advisory-db.
- Skipping `homing-preflight.sh` before first enable on bench.
