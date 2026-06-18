# Development setup

## Docker (recommended)

Use the container workflow unless you have a reason not to. Dev container setup: [onboarding.md](onboarding.md). Pi bring-up: [pi-commissioning.md](pi-commissioning.md).

```bash
docker compose build dev
just check          # or: docker compose run --rm check
```

Optional:

- Dev Container: Reopen in Container (Cursor / VS Code), [`.devcontainer/`](../.devcontainer/)
- SocketCAN test harness: `just vcan` (Linux, privileged; creates `vcan0`/`vcan1` as test stand-ins for production `can0`/`can1`)
- sim: `just sim-check`

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
| Node | 24.16.0 (see [.nvmrc](../.nvmrc), [mise.toml](../mise.toml); matches CI/dev container) |
| protoc | 28.3 |
| buf | 1.47.2 |

### Node on Mac / Windows (avoid lockfile drift)

CI and the dev container use Node 24 (currently 24.16.x). Use mise (or nvm/fnm) so Mac and Windows match CI. Do not rely on an unpinned system Node.

[mise](https://mise.jdx.dev/) (Mac + Windows + Linux):

```bash
# once per machine
mise trust
mise install          # reads mise.toml → Node 24.16.0, Rust, buf, protoc

# in repo root — verify
mise exec -- node -v    # v24.16.0
cd consul && mise exec -- npm ci
```

Alternatives: nvm / fnm read [.nvmrc](../.nvmrc) or [.node-version](../.node-version).

Consul `package-lock.json` rules:

| Task | Command |
|------|---------|
| Install deps (day-to-day) | `cd consul && mise exec -- npm ci` or `just consul-ci` |
| After editing `consul/package.json` | `just consul-lock` then commit lockfile |
| Never | `npm install` on Windows/Mac alone to refresh the lockfile — optional deps differ from Linux CI |

`consul/.npmrc` sets `engine-strict=true`; npm refuses Node outside `^24.16.0`.

```bash
# macOS examples (prefer `mise install` for pinned Node 24.16.0 — see above)
brew install rust protobuf bufbuild/buf/buf
git lfs install && git lfs pull

# Pi cross-build from Mac (one-time toolchain + deploy)
./scripts/setup-mac-pi-cross.sh
./scripts/deploy-pi.sh --install joey@marengo.local
# or: just deploy-pi

# Windows (no native aarch64 GCC): deploy via dev container (cached volumes + live logs)
# Set MARENGO_PI_HOST to a resolvable name (Tailscale / IP) — marengo.local mDNS fails inside Docker.
export MARENGO_PI_HOST=joey-robot.tail0b414.ts.net   # example
./scripts/deploy-pi-docker.sh
# or: just deploy-pi-docker
# Binary-only (faster): just deploy-pi-docker-binaries
# Verbose: MARENGO_DEPLOY_VERBOSE=1 just deploy-pi-docker

# WSL2 (recommended for daily work): native cross-build on ext4 — see docs/wsl-setup.md
./scripts/setup-wsl-pi-cross.sh
just deploy-pi-wsl

cargo build --workspace
cd consul && mise exec -- npm ci && npm run gen:proto
./scripts/check.sh
```

WSL2: clone inside the Linux filesystem (`~/code`), not `/mnt/c/`.

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
cd consul && npm update   # then: just consul-lock && just consul-ci
just check && just sim-check
```

Review `cargo audit` / `npm audit` output.

## Branch protection

On GitHub, require the **check** workflow to pass before merging to `main`.

## Patterns and safety

- [rust-patterns.md](rust-patterns.md)
- [safety.md](safety.md)
- [troubleshooting.md](troubleshooting.md)
