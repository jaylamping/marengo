# WSL2 development setup (Windows)

Run Marengo’s Linux toolchain **inside WSL2** for faster Docker/Rust I/O. Keep **SolidWorks + solidworks-mcp on Windows** when you need CAD.

## Why WSL2

| On `C:\code\...` (Windows filesystem) | On `~/code/...` (WSL ext4) |
|---------------------------------------|----------------------------|
| Slow bind mounts into containers | Fast native Linux I/O |
| More permission quirks (`EACCES`) | Fewer volume permission issues |
| vcan awkward | vcan works in Linux container |

**Rule:** clone under **`~/code`**, never **`/mnt/c/code`**.

---

## 1. Install WSL2 + Ubuntu

**PowerShell (Admin):**

```powershell
wsl --install
```

Reboot if prompted. Default distro is usually **Ubuntu**. Set username/password on first launch.

Verify:

```powershell
wsl -l -v
```

`VERSION` should be **2** for your distro.

Update inside WSL:

```bash
sudo apt update && sudo apt upgrade -y
sudo apt install -y git git-lfs build-essential
```

---

## 2. Docker Desktop + WSL integration

1. Install [Docker Desktop for Windows](https://docs.docker.com/desktop/setup/install/windows-install/) if not already.
2. **Settings → General:** enable **Use the WSL 2 based engine**.
3. **Settings → Resources → WSL integration:** enable your Ubuntu distro.
4. Apply & restart Docker Desktop.

Verify **inside WSL** (not PowerShell):

```bash
docker version
docker compose version
```

Both should succeed without `npipe` errors.

---

## 3. Clone Marengo in the Linux filesystem

```bash
mkdir -p ~/code
cd ~/code
git clone <repo-url> marengo
cd marengo
git lfs install
git lfs pull
```

Optional sibling for SolidWorks MCP (Windows path still used for CAD; see §7):

```bash
cd ~/code
git clone <solidworks-mcp-url> solidworks-mcp
```

---

## 4. Verify the workspace

From `~/code/marengo`:

```bash
docker compose build dev
docker compose run --rm check
```

Or install [just](https://github.com/casey/just) in WSL and run `just check`.

First build takes a while; later runs use cached Docker layers and named volumes (`target/`, `consul/node_modules`).

---

## 5. Cursor on WSL

### Option A — Open folder in WSL (recommended for daily Rust/docker work)

1. Install **WSL** extension in Cursor (usually bundled).
2. Command Palette → **WSL: Connect to WSL** (pick Ubuntu).
3. **File → Open Folder** → `/home/<you>/code/marengo`.
4. Terminal in Cursor runs in WSL automatically.

### Option B — Dev Container (same as today)

1. Open `~/code/marengo` in WSL.
2. Command Palette → **Dev Containers: Reopen in Container**.

Uses [`.devcontainer/devcontainer.json`](../.devcontainer/devcontainer.json) — same image as `docker compose`.

### Option C — Multi-root workspace (marengo + solidworks-mcp)

Open `marengo.code-workspace` from the WSL path:

```bash
cursor ~/code/marengo/marengo.code-workspace
```

---

## 6. Daily commands (WSL terminal)

```bash
cd ~/code/marengo

just check          # CI parity
just shell          # interactive dev container
just vcan           # virtual CAN (Linux)
just sim-check      # MuJoCo smoke tests
```

Inside dev container:

```bash
cargo build --workspace
cargo test --workspace
```

---

## 7. Hybrid: SolidWorks stays on Windows (one git tree)

SolidWorks and the MCP worker run on **Windows**. Software build/run uses **WSL**.
There is **one** Marengo git clone — on WSL ext4. See [ADR 0016](decisions/0016-wsl-software-home.md).

| Task | Where |
|------|--------|
| `just check`, Rust, Docker, marengo-pi MCP | WSL Cursor → `~/code/marengo` |
| SolidWorks CAD + solidworks-mcp | Windows Cursor → `\\wsl$\Ubuntu\home\<you>\code\marengo` |
| solidworks-mcp repo | Windows (`C:\code\solidworks-mcp`) |
| MCP CAD root | `${workspaceFolder}` in [`.cursor/mcp.json`](../.cursor/mcp.json) (UNC workspace when on Windows) |

**Do not** keep a second writeable clone at `C:\code\marengo`. That is how branch drift starts.

**Cursor on WSL** (daily software): **WSL: Connect** → `/home/<you>/code/marengo`.

**Cursor on Windows** (CAD only): open the same tree via UNC, e.g.
`\\wsl$\Ubuntu\home\<you>\code\marengo` (or that path’s `marengo.code-workspace`).

Pi MCP host defaults live in [`tools/marengo-pi-mcp/launch.mjs`](../tools/marengo-pi-mcp/launch.mjs) (and `run-mcp.sh` / `run-mcp.ps1`) — not as machine-specific env in `.cursor/mcp.json` (Cursor hashes that file’s env and can auto-disable servers).

---

## 8. Migrating from `C:\code\marengo`

If you still have a Windows clone:

1. Ensure WSL `~/code/marengo` is up to date (`git status`, pull/rebase as needed).
2. Copy any Windows-only uncommitted files into WSL (or discard them).
3. Rename the Windows tree out of the way, then delete after a short seatbelt:

```powershell
Rename-Item -Path C:\code\marengo -NewName marengo.DEAD
# After a week (or when sure): Remove-Item -Recurse -Force C:\code\marengo.DEAD
```

Do **not** work daily from `/mnt/c/code/marengo` — that path is still slow even before you retire it.

---

## 9. Optional native tools in WSL (without Docker)

Best-effort; container remains source of truth ([dev-setup.md](dev-setup.md)).

```bash
# One-shot: SSH config, git identity, cross-GCC, mise, Pi MCP build
./scripts/setup-wsl-dev.sh

# Or piecemeal:
./scripts/setup-wsl-pi-cross.sh
just deploy-pi-wsl
# Binary-only (skip consul): ./scripts/deploy-pi.sh --install --skip-consul joey@marengo.local

# mise (matches mise.toml + rust-toolchain.toml 1.88)
curl https://mise.run | sh
cd ~/code/marengo && mise install
```

After `setup-wsl-dev.sh`, **restart the marengo-pi MCP server** in Cursor.
Defaults (Pi host, SSH identity under `~/.ssh/`, bench profile) come from
`tools/marengo-pi-mcp/launch.mjs` / `run-mcp.sh`.

---

## 10. Troubleshooting

| Symptom | Fix |
|---------|-----|
| `failed to connect to docker API at npipe://...` | Start Docker Desktop; enable WSL integration; run commands **inside WSL**. Avoid a `docker` shim that only calls `docker.exe` once the unix socket exists. |
| `Permission denied (publickey)` to Pi | Ensure `~/.ssh/id_ed25519_marengo` + `~/.ssh/config` (`./scripts/setup-wsl-dev.sh`). Bare `ssh joey@marengo.local` must work. |
| `Author identity unknown` on commit | Set git `user.name` / `user.email` (or re-run `setup-wsl-dev.sh`). |
| Builds still slow | Confirm repo is `~/code/...`, not `/mnt/c/...`. Use `just deploy-pi-docker` (not bare `docker run`); 2nd run should reuse `cargo-target` / `cargo-registry` volumes. |
| Deploy looks hung, no output | Use `./scripts/deploy-pi-docker.sh` (line-buffered + `CARGO_TERM_PROGRESS_WHEN=always`). Verbose: `MARENGO_DEPLOY_VERBOSE=1 just deploy-pi-docker`. |
| Want native deploy without Docker | WSL2: `./scripts/setup-wsl-pi-cross.sh` (needs `libc6-dev-arm64-cross`, not just the GCC package) then `just deploy-pi-wsl`. |
| Cross-build fails: `bits/libc-header-start.h: No such file` | Install aarch64 libc headers: `sudo apt-get install -y libc6-dev-arm64-cross` (or re-run `./scripts/setup-wsl-pi-cross.sh`). |
| `git lfs` smudge errors | `git lfs install && git lfs pull` inside WSL clone. |
| Line-ending noise | In WSL clone: `git config core.autocrlf input`. |
| Permission errors on `target/` | `docker compose build dev` (entrypoint chowns volumes). See [troubleshooting.md](troubleshooting.md). |
| SolidWorks MCP can’t see files | Open Windows Cursor on `\\wsl$\...\code\marengo` so `${workspaceFolder}` matches the tree; restart solidworks MCP. |
| marengo-pi MCP missing / disabled in WSL | Usually disabled, not missing. Enable in MCP settings, or quit Cursor and run `just mcp-ensure-enabled --write`. `sessionStart` also runs a best-effort ensure hook (Node). Do **not** put profile/SSH env in `.cursor/mcp.json` (hash thrash auto-disables). Defaults are in `tools/marengo-pi-mcp/run-mcp.sh` (WSL entry) / `launch.mjs` (Node entry after resolve). |
| `spawn node ENOENT` / marengo-pi stuck disabled | Cursor's spawn PATH has no mise `node`. WSL software session must use `bash` + `tools/marengo-pi-mcp/run-mcp.sh` (resolves mise node) — **not** bare `node` + `launch.mjs`. Restart MCP after fixing mcp.json. Windows CAD: use `run-mcp.cmd` / `run-mcp.ps1` if needed. If the toggle is still off: Enable once in MCP settings (or quit Cursor and `just mcp-ensure-enabled --write`). |

---

## Quick reference

```bash
# One-time
wsl --install
# Docker Desktop: WSL integration ON
mkdir -p ~/code && cd ~/code && git clone <repo-url> marengo && cd marengo && git lfs pull

# Every session
cd ~/code/marengo && just check
```

See also [onboarding.md](onboarding.md), [dev-setup.md](dev-setup.md).
