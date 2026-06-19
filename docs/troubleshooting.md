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
| Tiny pointer files instead of ONNX | `git lfs install && git lfs pull` |
| SolidWorks files missing after clone | Expected — CAD is not in this repo. Restore locally per [cad/README.md](cad/README.md). |

## SocketCAN

| Symptom | Fix |
|---------|-----|
| `Cannot find device can0` | Bring up the production interface: `sudo ip link set can0 type can bitrate 1000000 && sudo ip link set can0 up`. Repeat for `can1`, `can2`, etc. |
| Waveshare 2-CH CAN HAT registers but CAN traffic goes `ERROR-WARNING` / `ERROR-PASSIVE` | Verify `/boot/firmware/config.txt` matches the Waveshare 2-CH CAN HAT wiki: `dtparam=spi=on`, `dtoverlay=mcp2515-can1,oscillator=16000000,interrupt=25`, `dtoverlay=mcp2515-can0,oscillator=16000000,interrupt=23`, and `dtoverlay=spi-bcm2835-overlay`. A wrong oscillator value can register `can0`/`can1` while producing bad bus timing. After reboot, `ip -details link show type can` should report `clock 8000000`. |
| Configured motor never appears | Check `config/motors.yaml` `can_interface` + `device_id`; runtime does not auto-register or prune devices. A silent configured motor is a safety fault. |
| Need raw CAN/router logs | Run with `RUST_LOG=robstride=trace,davout=debug,berthier=debug,marengo_pi=info` (or `motor_repl=info`) to log SocketCAN open, TX/RX frame IDs, decoded status, unknown frames, watchdog expiry, and motor faults. **Bring-up only** — revert to `scripts/env.example` defaults for normal bench. |
| Need position-hold motion debug | Set `MARENGO_POSITION_TRACE=/path/to/trace.csv` for high-rate CSV (side channel). Optional `berthier=debug` for onset burst logs. See [logging-taxonomy.md](logging-taxonomy.md). |
| Need live log-level changes from Consul | **Not implemented** (roadmap M5). Edit `RUST_LOG` in `/etc/marengo/env` and restart `marengo-pi` / `marengo-gateway`, or use per-crate overrides in `scripts/env.example`. |
| Need virtual CAN tests | Run `docker compose --profile vcan up -d vcan` or `just vcan` on Linux. This creates `vcan0`/`vcan1` only for tests. |
| vcan on macOS host | Use Linux container with `privileged: true`; SocketCAN is not available on macOS natively. |
| `modprobe vcan` fails in container | With `--network host`, the host must load vcan and create `vcan0`/`vcan1` first (`./scripts/ci-vcan-host-setup.sh` in CI, or `./scripts/vcan-up.sh` on Linux). Inside the container, `vcan-up` probes `ip link type vcan` when `/lib/modules` is missing — CAN tests still fail if vcan is unavailable. |

## Cross-compile (Pi / Jetson)

| Symptom | Fix |
|---------|-----|
| `linker aarch64-linux-gnu-gcc not found` | On Mac: `./scripts/setup-mac-pi-cross.sh` then `just deploy-pi`. On Windows: `just deploy-pi-docker` (or `./scripts/deploy-pi-docker.sh`). |
| `cargo: command not found` in Docker deploy | Fixed: run through `./scripts/deploy-pi-docker.sh` (uses entrypoint + `PATH=/usr/local/cargo/bin`). Do not wrap in `bash -lc` manually. |
| `failed to get console: provided file is not a console` / `creating a console from a file is not supported on windows` / `The handle is invalid` | Docker `--ansi always`, `-t`, or `tty: true` on Windows PowerShell → Docker Desktop. `deploy-pi-docker.sh` uses plain `docker compose` + `-T`; `deploy-pi` service has no `tty:`. Progress via `CARGO_TERM_PROGRESS_WHEN=always`. |
| `exec format error` during `docker compose build` | Wrong platform image (e.g. arm64 on x64). Compose sets `platform: linux/amd64`; override with `DOCKER_PLATFORM=linux/arm64` on Apple Silicon if needed. |
| `can't find crate for std` / `target may not be installed` | Run `rustup target add aarch64-unknown-linux-gnu` once in the dev container, or use `deploy-pi-docker` / `ensure_pi_cross_target` in `deploy-pi.sh`. |
| `pi_build` / `pi_sync_main pi_native`: `cargo: command not found` on Pi | Install Rust on Pi or use cross deploy. MCP now sources `~/.cargo/env` and prepends `~/.cargo/bin` on SSH. |
| `deploy-pi.sh` / `buf` `ENOENT` on `downloaded-*-linux-x64-buf` | Host `consul/node_modules` from Windows npm inside Linux Docker — `deploy-pi.sh` re-runs `npm ci` when buf fails; prefer `just deploy-pi-docker` (uses Linux `consul-node-modules` volume). |
| Deploy re-downloads all crates every time | Bare `docker run -v .:/workspace` skips named volumes. Use `just deploy-pi-docker` only. First run ~2–5 min; incremental runs reuse `cargo-target` + `cargo-registry`. |
| Deploy shows no output / looks frozen | PowerShell pipes buffer Docker. Run `just deploy-pi-docker` unfiltered; set `MARENGO_DEPLOY_VERBOSE=1` for npm/cargo detail. Steps log as `==> [mm:ss] …`. |
| `Could not resolve hostname marengo.local` in Docker deploy | mDNS does not resolve inside the container. Set `MARENGO_PI_HOST` to a reachable name (e.g. Tailscale `joey-robot.tail….ts.net` or Pi IP) — same as `MARENGO_PI_HOST` in `.cursor/mcp.json`. |
| Cloud agent cannot reach Pi | Tailscale userspace mode required. Add secrets per [cloud-pi-tailscale.md](cloud-pi-tailscale.md); run `./scripts/setup-cloud-pi.sh --verify`. Use `./scripts/pi-remote.sh` instead of marengo-pi MCP. |
| `Permission denied (publickey)` in Docker deploy | Windows bind-mounts `~/.ssh` with loose permissions; `deploy-pi.sh` copies keys to `/tmp` with mode 600, prefers `id_ed25519_marengo`, and sets `IdentitiesOnly=yes`. If the mount is empty, Git Bash pathconv broke the `-v` bind — `deploy-pi-docker.sh` uses `//c/...` paths and `MSYS_NO_PATHCONV=1`. |
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
