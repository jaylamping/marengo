# Development setup

## Recommended: Docker (source of truth)

See [onboarding.md](onboarding.md) for dev container setup. For **physical Pi bring-up**, see [pi-commissioning.md](pi-commissioning.md).

```bash
docker compose build dev
just check          # or: docker compose run --rm check
```

Optional:

- **Dev Container:** Reopen in Container (Cursor / VS Code) — [`.devcontainer/`](../.devcontainer/)
- **SocketCAN test harness:** `just vcan` (Linux, privileged; creates `vcan0`/`vcan1` as test stand-ins for production `can0`/`can1`)
- **sim:** `just sim-check`

Production runtime uses SocketCAN interfaces named `can0`, `can1`, `can2`, etc. Bring them up on the robot before starting `marengo-pi`, for example:

```bash
sudo ip link set can0 type can bitrate 1000000
sudo ip link set can0 up
```

## Native / host (best-effort)

If you cannot use Docker, install tools matching [mise.toml](../mise.toml):

| Tool | Version |
|------|---------|
| Rust | 1.88 (see [rust-toolchain.toml](../rust-toolchain.toml)) |
| Node | 22 |
| protoc | 28.3 |
| buf | 1.47.2 |

```bash
# macOS examples
brew install rust protobuf bufbuild/buf/buf node@22
git lfs install && git lfs pull

# Pi cross-build from Mac (one-time toolchain + deploy)
./scripts/setup-mac-pi-cross.sh
./scripts/deploy-pi.sh --install joey@marengo.local
# or: just deploy-pi

cargo build --workspace
cd consul && npm ci && npm run gen:proto
./scripts/check.sh
```

**WSL2:** clone inside the Linux filesystem (`~/code`), not `/mnt/c/`.

## Regenerating wire types

1. Edit `proto/*.proto`.
2. Rust: `cargo build -p armee-proto`.
3. TypeScript: `cd consul && npm run gen:proto`.
4. Update checksum: `shasum -a 256 consul/src/gen/marengo/v1/marengo_pb.ts | awk '{print $1}' > consul/src/gen/.checksum`

Never commit hand-edits to `consul/src/gen/` (gitignored except `.checksum`).

## Dependency updates (manual — no Dependabot)

Monthly (or before releases):

```bash
cargo update
cd consul && npm update
just check && just sim-check
```

Review `cargo audit` / `npm audit` output.

## Branch protection

On GitHub, require the **check** workflow to pass before merging to `main`.

## Patterns and safety

- [rust-patterns.md](rust-patterns.md)
- [safety.md](safety.md)
- [troubleshooting.md](troubleshooting.md)
