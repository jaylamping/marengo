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
