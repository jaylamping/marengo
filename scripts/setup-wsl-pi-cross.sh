#!/usr/bin/env bash
# One-time WSL2 / Debian setup for native Pi cross-build (no Docker deploy).
set -euo pipefail

if [[ -f /proc/version ]] && grep -qi microsoft /proc/version; then
  : # WSL
elif [[ "$(uname -s)" != Linux ]]; then
  echo "setup-wsl-pi-cross: requires Linux or WSL2" >&2
  exit 1
fi

# Compiler + aarch64 libc headers (ring/openssl need bits/libc-header-start.h).
need_pkgs=()
command -v aarch64-linux-gnu-gcc >/dev/null 2>&1 || need_pkgs+=(gcc-aarch64-linux-gnu)
command -v aarch64-linux-gnu-g++ >/dev/null 2>&1 || need_pkgs+=(g++-aarch64-linux-gnu)
# Header probe: host /usr/include/stdint.h is wrong for cross; sysroot must exist.
if [[ ! -f /usr/aarch64-linux-gnu/include/bits/libc-header-start.h ]] \
  && [[ ! -f /usr/include/aarch64-linux-gnu/bits/libc-header-start.h ]]; then
  need_pkgs+=(libc6-dev-arm64-cross)
fi

if [[ ${#need_pkgs[@]} -gt 0 ]]; then
  echo "Installing cross packages (sudo): ${need_pkgs[*]}"
  sudo apt-get update
  sudo apt-get install -y --no-install-recommends "${need_pkgs[@]}"
fi

if ! rustup target list --installed | grep -q '^aarch64-unknown-linux-gnu$'; then
  echo "Adding Rust target aarch64-unknown-linux-gnu..."
  rustup target add aarch64-unknown-linux-gnu
fi

# Ensure cargo uses the cross linker (repo .cargo/config.toml may already set this).
mkdir -p "${HOME}/.cargo"
CFG="${HOME}/.cargo/config.toml"
if ! grep -q 'aarch64-unknown-linux-gnu' "$CFG" 2>/dev/null; then
  cat >>"$CFG" <<'EOF'

[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
EOF
  echo "Appended [target.aarch64-unknown-linux-gnu] linker to ${CFG}"
fi

echo "setup-wsl-pi-cross: ok"
echo "  gcc: $(command -v aarch64-linux-gnu-gcc)"
echo "Deploy: just deploy-pi-wsl"
