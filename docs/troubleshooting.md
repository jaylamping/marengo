# Troubleshooting

## Container / `just check`

| Symptom | Fix |
|---------|-----|
| `protoc: not found` | Use the dev container or `docker compose run --rm check` — do not rely on host protoc. |
| `buf: not found` | Run checks inside the container; for native dev, `cd consul && npm ci`. |
| Permission denied on `target/` | Files owned by root from Docker — `sudo chown -R $(id -u):$(id -g) target` or use the `marengo` user in the image. |
| Slow builds on macOS | Ensure the repo is not on a slow bind mount; use named volumes for `target/` (see `compose.yaml`). |

## Git LFS

| Symptom | Fix |
|---------|-----|
| Tiny pointer files instead of CAD/meshes | `git lfs install && git lfs pull` |
| CI fails on missing assets | Add LFS pull to bootstrap; verify `.gitattributes` patterns. |

## vcan (virtual CAN)

| Symptom | Fix |
|---------|-----|
| `Cannot find device vcan0` | Run `docker compose --profile vcan up -d vcan` or `just vcan` on Linux. |
| vcan on macOS host | Use Linux container with `privileged: true`; SocketCAN is not available on macOS natively. |
| `modprobe: not found` | Use the `vcan` compose service (requires Linux, privileged). |

## Cross-compile (Pi / Jetson)

| Symptom | Fix |
|---------|-----|
| `linker aarch64-linux-gnu-gcc not found` | Build inside `marengo-dev` image (includes cross toolchain). |
| Wrong glibc on device | Match Debian/bookworm-based images to Pi OS / JetPack versions. |

## Proto / Consul codegen

| Symptom | Fix |
|---------|-----|
| TypeScript errors in `consul/` | `cd consul && npm run gen:proto` — never hand-edit `src/gen/`. |
| Rust build fails after `.proto` change | `cargo build -p armee-proto` regenerates via `prost-build`. |
| Checksum mismatch in CI | Regenerate TS (`npm run gen:proto`) and update `consul/src/gen/.checksum`. |
| Missing `.checksum` in CI | Run `npm run gen:proto` then `shasum -a 256 consul/src/gen/marengo_pb.ts \| awk '{print $1}' > consul/src/gen/.checksum` and commit. |

## Simulation

| Symptom | Fix |
|---------|-----|
| `check-sim` skipped | Run `docker compose --profile sim run --rm check-sim` (requires `Dockerfile.sim`). |
| MuJoCo model load fails | Verify `MARENGO_SIM_MODEL` points at `sim/fixtures/minimal.xml`. |
