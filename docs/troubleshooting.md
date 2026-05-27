# Troubleshooting

## Container / `just check`

| Symptom | Fix |
|---------|-----|
| `protoc: not found` | Use the dev container or `docker compose run --rm check` — do not rely on host protoc. |
| `buf: not found` | Run checks inside the container; for native dev, `cd consul && npm ci`. |
| Permission denied on `target/` | Named volumes owned by root — rebuild dev image and re-run; entrypoint chowns `target/` and `consul/node_modules`. Or `docker compose run --rm --user root dev chown -R marengo:marengo /workspace/target /workspace/consul/node_modules`. |
| `EACCES` on `consul/node_modules` during `just check` | Same as above — ensure `docker/Dockerfile.dev` entrypoint is active (rebuild: `docker compose build dev`). |
| Slow builds on macOS | Ensure the repo is not on a slow bind mount; use named volumes for `target/` (see `compose.yaml`). |

## Git LFS

| Symptom | Fix |
|---------|-----|
| Tiny pointer files instead of CAD/meshes | `git lfs install && git lfs pull` |
| CI fails on missing assets | Add LFS pull to bootstrap; verify `.gitattributes` patterns. |

## SocketCAN

| Symptom | Fix |
|---------|-----|
| `Cannot find device can0` | Bring up the production interface: `sudo ip link set can0 type can bitrate 1000000 && sudo ip link set can0 up`. Repeat for `can1`, `can2`, etc. |
| Waveshare 2-CH CAN HAT registers but CAN traffic goes `ERROR-WARNING` / `ERROR-PASSIVE` | Verify `/boot/firmware/config.txt` matches the Waveshare 2-CH CAN HAT wiki: `dtparam=spi=on`, `dtoverlay=mcp2515-can1,oscillator=16000000,interrupt=25`, `dtoverlay=mcp2515-can0,oscillator=16000000,interrupt=23`, and `dtoverlay=spi-bcm2835-overlay`. A wrong oscillator value can register `can0`/`can1` while producing bad bus timing. After reboot, `ip -details link show type can` should report `clock 8000000`. |
| Configured motor never appears | Check `config/motors.yaml` `can_interface` + `device_id`; runtime does not auto-register or prune devices. A silent configured motor is a safety fault. |
| Need raw CAN/router logs | Run with `RUST_LOG=robstride=trace,davout=debug,berthier=debug,marengo_pi=info` (or `motor_repl=info`) to log SocketCAN open, TX/RX frame IDs, decoded status, unknown frames, watchdog expiry, and motor faults. |
| Need virtual CAN tests | Run `docker compose --profile vcan up -d vcan` or `just vcan` on Linux. This creates `vcan0`/`vcan1` only for tests. |
| vcan on macOS host | Use Linux container with `privileged: true`; SocketCAN is not available on macOS natively. |
| `modprobe: not found` | Use the `vcan` compose service (requires Linux, privileged). |

## Cross-compile (Pi / Jetson)

| Symptom | Fix |
|---------|-----|
| `linker aarch64-linux-gnu-gcc not found` | On Mac: `./scripts/setup-mac-pi-cross.sh` then `just deploy-pi`. On Windows: `just deploy-pi-docker` (or `./scripts/deploy-pi-docker.sh`). |
| `deploy-pi.sh` / `buf` `ENOENT` on `downloaded-*-linux-x64-buf` | Host `consul/node_modules` from Windows npm inside Linux Docker — `deploy-pi.sh` re-runs `npm ci` when buf fails; prefer `just deploy-pi-docker` (uses Linux `consul-node-modules` volume). |
| Deploy re-downloads all crates every time | Bare `docker run -v .:/workspace` skips named volumes. Use `just deploy-pi-docker` only. First run ~2–5 min; incremental runs reuse `cargo-target` + `cargo-registry`. |
| Deploy shows no output / looks frozen | PowerShell pipes buffer Docker. Run `just deploy-pi-docker` unfiltered; set `MARENGO_DEPLOY_VERBOSE=1` for npm/cargo detail. Steps log as `==> [mm:ss] …`. |
| Wrong glibc on device | Match Debian/bookworm-based images to Pi OS / JetPack versions. |

## Proto / Consul codegen

| Symptom | Fix |
|---------|-----|
| TypeScript errors in `consul/` | `cd consul && npm run gen:proto` — never hand-edit `src/gen/`. |
| Rust build fails after `.proto` change | `cargo build -p armee-proto` regenerates via `prost-build`. |
| Checksum mismatch in CI | Regenerate TS (`npm run gen:proto`) and update `consul/src/gen/.checksum`. |
| Missing `.checksum` in CI | Run `npm run gen:proto` then `shasum -a 256 consul/src/gen/marengo/v1/marengo_pb.ts \| awk '{print $1}' > consul/src/gen/.checksum` and commit. |

## Simulation

| Symptom | Fix |
|---------|-----|
| `check-sim` skipped | Run `docker compose --profile sim run --rm check-sim` (requires `Dockerfile.sim`). |
| MuJoCo model load fails | Verify `MARENGO_SIM_MODEL` points at `sim/fixtures/minimal.xml`. |
