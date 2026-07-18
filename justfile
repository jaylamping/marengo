# Marengo task runner — https://github.com/casey/just

default:
    @just --list

# Build dev image
build:
    docker compose build dev

# CI-parity checks (container)
check: build
    docker compose run --rm check

# Fast container bootstrap
bootstrap: build
    docker compose run --rm dev ./scripts/bootstrap.sh

# Interactive dev shell
shell: build
    docker compose run --rm dev

# Virtual CAN (Linux, privileged)
vcan: build
    docker compose --profile vcan up -d vcan

# vcan integration tests
check-vcan: vcan
    docker compose --profile vcan run --rm check-vcan

# MuJoCo simulation checks
sim-check: build
    docker compose build check-sim
    docker compose --profile sim run --rm check-sim

# Host-native check (requires local toolchain; cloud: run setup-cloud.sh first)
check-native:
    ./scripts/check.sh

# Cloud agent: Tailscale + SSH to Pi (see docs/cloud-pi-tailscale.md)
cloud-pi-setup:
    ./scripts/setup-cloud-pi.sh

cloud-pi-verify:
    ./scripts/setup-cloud-pi.sh --verify

# Cloud/local Pi CLI when marengo-pi MCP unavailable
pi-remote *args:
    ./scripts/pi-remote.sh {{args}}

# Cross-build and deploy Pi binaries from macOS (installs cross GCC if needed)
deploy-pi host="joey@marengo.local":
    ./scripts/deploy-pi.sh --install {{host}}

# Cross-build + deploy via dev container (Windows / no native aarch64 GCC).
deploy-pi-docker host="joey@marengo.local":
    ./scripts/deploy-pi-docker.sh {{host}}

# Fast binary-only deploy via Docker (skip consul npm build).
deploy-pi-docker-binaries host="joey@marengo.local":
    MARENGO_SKIP_CONSUL=1 ./scripts/deploy-pi-docker.sh {{host}}

# WSL2 native cross-build (no Docker) — run once: ./scripts/setup-wsl-pi-cross.sh
deploy-pi-wsl host="joey@marengo.local":
    ./scripts/deploy-pi.sh --install {{host}}

# Rebuild Marengo MCP servers (Cursor: restart MCP after this)
mcp-build:
    cd tools/marengo-pi-mcp && npm install && npm run build

# Install Marengo Research MCP (Python/uv; restart marengo-research MCP after this)
research-mcp-setup:
    cd tools/marengo-research-mcp && uv sync --extra dev

# Local deterministic daily audit dry-run (full pipeline via run.sh)
daily-audit:
    ./scripts/daily-audit/run.sh

# Regenerate consul/package-lock.json in the Linux dev container (matches CI npm ci).
consul-lock:
    docker compose run --rm dev bash -c "cd consul && npm install --package-lock-only"

# Install consul deps exactly as CI (Linux Node 24 npm ci).
consul-ci:
    docker compose run --rm dev bash -c "cd consul && npm ci && npm audit --audit-level=high"
