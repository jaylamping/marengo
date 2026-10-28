# scripts/ — Build, deploy, CI, bench tooling

Shell + Python scripts. No Rust logic here — these orchestrate the workspace, deploy to the Pi, and provide bench diagnostics.

## STRUCTURE

```
scripts/
├── check.sh                  # CI-parity gate (`just check` in Docker, `just check-native` on host)
├── check-dependencies.sh     # cargo deny + cargo audit with fresh advisory DB
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
├── deploy-lib.sh             # Shared deploy logging helpers (sourced)
├── pi-self-update.sh         # Pi-native self-update: pin SHA → build → install (MCP pi_native, gateway)
├── pi-enqueue-self-update.sh # Root helper: enqueue self-update outside the gateway cgroup
├── pi-restart-marengo-pi.sh  # Allowed sudo helper to stop/restart the marengo-pi unit
├── build-consul-native.sh    # Build consul/dist on the Pi for /opt/marengo/www
├── install-pi.sh             # Install staging tree → /opt/marengo on Pi
├── preserve-taught-limits.py # Keep Set Limits hard/soft/URDF across install rsync
├── pi-remote.sh              # Cloud fallback: SSH to Pi for health/logs/deploy
├── cloud-pi-lib.sh           # Shared Tailscale + SSH helpers for cloud agents (sourced)
├── pi-native-build.sh        # Native cargo build on Pi
├── pi-bno085-shtp-init.py    # BNO085 SHTP init verification
├── pi-i2c-plain-read.py      # Plain I2C read test (not smbus)
├── pi-i2c-raw-read.py        # Raw I2C read test
├── pi-bno085-smoke.py        # One-shot BNO085 smoke test (CircuitPython)
├── setup-cloud.sh            # Cloud VM: protoc + pinned cargo-deny/cargo-audit tools
├── setup-cloud-pi.sh         # Cloud VM: Tailscale + SSH to Pi
├── setup-mac-pi-cross.sh     # macOS aarch64 cross-compile setup
├── setup-wsl-dev.sh          # Legacy WSL bootstrap (ADR 0018 retired the WSL checkout)
├── setup-wsl-pi-cross.sh     # Legacy WSL2 cross-compile setup (ADR 0018)
├── ensure-marengo-pi-mcp-enabled.py # Keep marengo-pi MCP enabled in Cursor workspace storage
├── reference-journal-tail.py # Read-only latest reference journal rows (history, never a grant)
├── log-inventory.sh          # Bench log inventory
├── bench-log-archive.sh      # Archive bench sessions
├── bench-log-prune.sh        # Prune old bench logs
├── bench-set-weighted-mass.sh # Set weighted-arm mass for bench tests
├── measure-can-mit-rate.sh   # CAN MIT frame rate measurement
├── analyze-position-trace.py # Parse position traces; --score-bench = ADR 0039 bench pass/fail
├── profile-pi-loop.sh        # Pi control loop profiling
├── ci-vcan-host-setup.sh     # CI: vCAN host setup
├── test-compose-ssh.sh       # Docker compose SSH test
├── *.test.sh, test_*.py      # Shell contract tests and Python unittest/pytest suites (run by check.sh)
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
- `check-dependencies.sh` requires a current advisory database/index refresh and treats incomplete scanner coverage as a failure. Keep its scanner versions aligned between Docker and cloud setup.
- Pi scripts assume Tailscale SSH in cloud, `marengo.local` mDNS on LAN.
- `pi-remote.sh` is the cloud fallback when marengo-pi MCP is unavailable.
- systemd units in `systemd/` are the production runtime definitions.
- Python diagnostics use plain stdlib — no venv required. The two gated test files need more: `test_analyze_position_trace.py` (pytest) and `test_preserve_taught_limits.py` (pyyaml); `check.sh` fails with the install command when they are missing.

## ANTI-PATTERNS

- Asking user to paste Pi logs when `pi-remote.sh` can fetch them.
- Suppressing advisory refresh or registry-index failures to make a dependency check pass.
- Do not use a fresh `motor-repl` process as a reference/readiness check: ADR 0036 grants are process-local. Acquire reference and enable inside one `marengo-pi` (`home <joints> sign-tested`, see `docs/homing.md`); use `pi_homing_status` for live state while that owner runs.
