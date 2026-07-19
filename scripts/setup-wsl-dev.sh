#!/usr/bin/env bash
# One-shot WSL session bootstrap for Marengo software work.
# Run from repo root: ./scripts/setup-wsl-dev.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ok() { printf '  OK  %s\n' "$*"; }
warn() { printf '  !!  %s\n' "$*" >&2; }
fail() { printf '  FAIL %s\n' "$*" >&2; }

echo "==> Marengo WSL bootstrap"

# --- SSH ---
mkdir -p "${HOME}/.ssh"
chmod 700 "${HOME}/.ssh"
if [[ ! -f "${HOME}/.ssh/id_ed25519_marengo" ]]; then
  if [[ -f /mnt/c/Users/joeyl/.ssh/id_ed25519_marengo ]]; then
    cp /mnt/c/Users/joeyl/.ssh/id_ed25519_marengo "${HOME}/.ssh/"
    cp /mnt/c/Users/joeyl/.ssh/id_ed25519_marengo.pub "${HOME}/.ssh/" 2>/dev/null || true
    chmod 600 "${HOME}/.ssh/id_ed25519_marengo"
    ok "copied id_ed25519_marengo from Windows .ssh"
  else
    fail "missing ~/.ssh/id_ed25519_marengo (copy from Windows ~/.ssh)"
  fi
else
  ok "SSH key present"
fi

if [[ ! -f "${HOME}/.ssh/config" ]]; then
  cat >"${HOME}/.ssh/config" <<'EOF'
Host marengo.local marengo
  Hostname marengo.local
  User joey
  IdentityFile ~/.ssh/id_ed25519_marengo
  IdentitiesOnly yes

Host marengo-ts joey-robot joey-robot.tail0b414.ts.net
  Hostname joey-robot.tail0b414.ts.net
  User joey
  IdentityFile ~/.ssh/id_ed25519_marengo
  IdentitiesOnly yes
EOF
  chmod 600 "${HOME}/.ssh/config"
  ok "wrote ~/.ssh/config"
else
  ok "SSH config present"
fi

if ssh -o BatchMode=yes -o ConnectTimeout=5 joey@marengo.local 'echo ok' >/dev/null 2>&1; then
  ok "ssh joey@marengo.local"
else
  warn "ssh joey@marengo.local failed — check Tailscale / key authorized on Pi"
fi

# --- Git identity (from Windows profile defaults) ---
if [[ -z "$(git config --global user.name || true)" ]]; then
  git config --global user.name "Joey Lamping"
  ok "set git user.name"
fi
if [[ -z "$(git config --global user.email || true)" ]]; then
  git config --global user.email "35737718+jaylamping@users.noreply.github.com"
  ok "set git user.email"
fi
git config --global core.autocrlf input || true

# --- Cross GCC (native Pi deploy without Docker) ---
if command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
  ok "aarch64-linux-gnu-gcc"
else
  echo "Installing aarch64-linux-gnu-gcc (sudo)…"
  # Call via bash so a missing +x bit cannot fail the bootstrap.
  bash "${ROOT}/scripts/setup-wsl-pi-cross.sh"
fi

# --- Rust pin ---
if command -v mise >/dev/null 2>&1; then
  mise trust >/dev/null 2>&1 || true
  mise install
  ok "mise install (rust 1.88 / node 24)"
fi
(cd "$ROOT" && unset RUSTUP_TOOLCHAIN && rustc --version)

# --- Docker ---
if docker version >/dev/null 2>&1; then
  ok "docker engine reachable"
else
  warn "docker not reachable — start Docker Desktop, enable WSL integration for this distro"
  warn "then re-run: docker version"
fi

# --- MCP ---
if [[ -f "${ROOT}/tools/marengo-pi-mcp/package.json" ]]; then
  (cd "${ROOT}/tools/marengo-pi-mcp" && npm install --silent && npm run build --silent)
  ok "marengo-pi-mcp built"
fi

echo
echo "Next:"
echo "  1. Restart Cursor MCP servers (marengo-pi should appear)"
echo "  2. Deploy: just deploy-pi-wsl"
echo "     or with Docker: just deploy-pi-docker host=joey@marengo.local"
echo "  3. CI parity: just check"
