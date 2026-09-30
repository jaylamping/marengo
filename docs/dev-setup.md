# Development setup

## Docker (recommended)

Open the host checkout (`J:\code\marengo` on Windows) and use Docker for full workspace checks. [Windows and macOS development](windows-macos-development.md) describes native frontend work, CAD, and Pi deployment. Dev container setup: [onboarding.md](onboarding.md). Pi bring-up: [pi-commissioning.md](pi-commissioning.md).

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

cargo build --workspace
cd consul && mise exec -- npm ci && npm run gen:proto
./scripts/check.sh
```

Windows Pi cross-builds use the native PowerShell wrapper and Docker caches:

```powershell
Set-Location J:\code\marengo
$env:MARENGO_PI_HOST = 'joey-robot.tail0b414.ts.net'
powershell -NoProfile -File .\scripts\deploy-pi-docker.ps1
```

Use a resolvable Tailscale name or IP because mDNS may not resolve inside Docker. Full native Windows Rust builds are currently blocked by Chappe's Unix IPC types; `just check` supplies the Linux environment. Legacy WSL scripts remain available for explicitly requested use.

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
