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

# Host-native check (requires local toolchain)
check-native:
    ./scripts/check.sh

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

# Rebuild Marengo Pi MCP server (Cursor: restart marengo-pi MCP after this)
mcp-build:
    cd tools/marengo-pi-mcp && npm install && npm run build
