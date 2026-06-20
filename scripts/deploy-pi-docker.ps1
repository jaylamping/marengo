#requires -Version 5.1
<#
.SYNOPSIS
  Pi deploy via dev container (Windows / hosts without aarch64 cross GCC).
  PowerShell port of scripts/deploy-pi-docker.sh.

.DESCRIPTION
  Uses compose named volumes for target/, cargo registry, and consul/node_modules
  so repeat deploys reuse compiled artifacts. Do NOT substitute bare
  `docker run -v .:/workspace` — that re-downloads crates and reinstalls npm
  every time.

  Runs `docker compose` directly from PowerShell, where Docker Desktop's
  npipe context (desktop-linux) works natively. This avoids the "protocol not
  available" failure that occurs when bash on Windows invokes docker through
  an incompatible context.

.PARAMETER PiHost
  user@host for the Pi deploy target. Defaults to MARENGO_PI_HOST /
  MARENGO_PI_USER env vars, or joey@marengo.local.
#>
[CmdletBinding()]
param(
  [Parameter(Position = 0)]
  [string]$PiHost
)

$ErrorActionPreference = "Stop"

# --- paths / host resolution -------------------------------------------------

$Root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
Set-Location -LiteralPath $Root

if (-not $PiHost) {
  $piHostEnv = $env:MARENGO_PI_HOST
  $piUserEnv = $env:MARENGO_PI_USER
  if ($piHostEnv -and $piHostEnv -match "@") {
    $PiHost = $piHostEnv
  } elseif ($piHostEnv) {
    $user = if ($piUserEnv) { $piUserEnv } else { "joey" }
    $PiHost = "$user@$piHostEnv"
  } else {
    $user = if ($piUserEnv) { $piUserEnv } else { "joey" }
    $PiHost = "$user@marengo.local"
  }
}

# --- ssh dir -----------------------------------------------------------------

$SshDir = if ($env:MARENGO_SSH_DIR) {
  $env:MARENGO_SSH_DIR -replace "\\", "/"
} else {
  (Join-Path $env:USERPROFILE ".ssh") -replace "\\", "/"
}

function Test-SshIdentity($Dir) {
  foreach ($k in @("id_ed25519_marengo", "id_ed25519", "id_rsa")) {
    if (Test-Path -LiteralPath (Join-Path $Dir $k)) { return $true }
  }
  return $false
}

if (-not (Test-Path -LiteralPath $SshDir) -or -not (Test-SshIdentity $SshDir)) {
  Write-Warning "SSH preflight: cannot find deploy key in $SshDir (continuing — Docker mount may still work)"
}

$env:MARENGO_SSH_DIR = $SshDir

# Docker Desktop on Windows accepts the Windows path for bind mounts
# when invoked from PowerShell (no MSYS path conversion needed).
$DockerSshMount = $SshDir -replace "/", "\"

# --- progress env ------------------------------------------------------------

if (-not $env:CARGO_TERM_PROGRESS_WHEN) { $env:CARGO_TERM_PROGRESS_WHEN = "auto" }
if (-not $env:CARGO_TERM_COLOR) { $env:CARGO_TERM_COLOR = "always" }
if (-not $env:NPM_CONFIG_PROGRESS) { $env:NPM_CONFIG_PROGRESS = "true" }
if (-not $env:NPM_CONFIG_LOGLEVEL) { $env:NPM_CONFIG_LOGLEVEL = "notice" }
if ($env:MARENGO_DEPLOY_VERBOSE -eq "1") {
  $env:NPM_CONFIG_LOGLEVEL = "verbose"
  if (-not $env:CARGO_LOG) { $env:CARGO_LOG = "cargo::core::compiler::fingerprint=info" }
}

$DockerPlatform = if ($env:DOCKER_PLATFORM) { $env:DOCKER_PLATFORM } else { "linux/amd64" }
if (-not $env:DOCKER_DEFAULT_PLATFORM) { $env:DOCKER_DEFAULT_PLATFORM = $DockerPlatform }

$DeployStart = Get-Date

function Write-Step([string]$Msg) {
  $elapsed = (Get-Date) - $DeployStart
  $stamp = "{0:00}:{1:00}" -f [int]$elapsed.TotalMinutes, $elapsed.Seconds
  Write-Host "`n==> [$stamp] $Msg" -ForegroundColor Cyan
}
function Write-Note([string]$Msg) {
  Write-Host "    $Msg" -ForegroundColor DarkGray
}
function Write-Warn([string]$Msg) {
  $elapsed = (Get-Date) - $DeployStart
  $stamp = "{0:00}:{1:00}" -f [int]$elapsed.TotalMinutes, $elapsed.Seconds
  Write-Host "warn [$stamp] $Msg" -ForegroundColor Yellow
}

Write-Step "deploy-pi-docker → $PiHost"
Write-Note "SSH mount: $DockerSshMount → /home/marengo/.ssh"
Write-Note "Platform: $DockerPlatform"
Write-Note "Cache volumes: cargo-target, cargo-registry, cargo-git, consul-node-modules"
Write-Note "Live logs: CARGO_TERM_PROGRESS_WHEN=$($env:CARGO_TERM_PROGRESS_WHEN)"
Write-Note "Verbose npm/cargo: MARENGO_DEPLOY_VERBOSE=$(if ($env:MARENGO_DEPLOY_VERBOSE) { $env:MARENGO_DEPLOY_VERBOSE } else { '' })"
Write-Note "Start UTC: $(Get-Date -Format 'yyyy-MM-ddTHH:mm:ssZ')"

# --- ensure image ------------------------------------------------------------

function Ensure-DeployImage {
  if ($env:MARENGO_FORCE_IMAGE_BUILD -eq "1") {
    Write-Step "docker compose build deploy-pi (forced)"
    & docker compose build deploy-pi 2>&1 | ForEach-Object { Write-Host $_ }
    if ($LASTEXITCODE -ne 0) { throw "docker compose build failed" }
    return
  }

  & docker image inspect marengo-dev:local 2>&1 | Out-Null
  if ($LASTEXITCODE -eq 0) {
    Write-Note "Reusing marengo-dev:local (set MARENGO_FORCE_IMAGE_BUILD=1 to rebuild)"
    return
  }

  Write-Step "docker compose build deploy-pi (image missing)"
  & docker compose build deploy-pi 2>&1 | ForEach-Object { Write-Host $_ }
  if ($LASTEXITCODE -ne 0) {
    throw "marengo-dev:local missing and docker compose build failed"
  }
}

Ensure-DeployImage

# --- run compose -------------------------------------------------------------

$deployArgs = @("./scripts/deploy-pi.sh", "--install", $PiHost)
if ($env:MARENGO_SKIP_CONSUL -eq "1") {
  $deployArgs = @("./scripts/deploy-pi.sh", "--install", "--skip-consul", $PiHost)
}

# Deploy is non-interactive; do not pass --ansi always or -t: Docker Desktop
# on Windows PowerShell fails with "creating a console from a file is not supported".

Write-Step "docker compose run deploy-pi"

$composeArgs = @(
  "compose", "run", "--rm", "-T",
  "-e", "MARENGO_DEPLOY_VIA_COMPOSE=1",
  "-e", "MARENGO_PI_HOST=$env:MARENGO_PI_HOST",
  "-e", "MARENGO_PI_USER=$env:MARENGO_PI_USER",
  "-e", "MARENGO_SKIP_CONSUL=$env:MARENGO_SKIP_CONSUL",
  "-e", "CARGO_TERM_PROGRESS_WHEN=$env:CARGO_TERM_PROGRESS_WHEN",
  "-e", "CARGO_TERM_COLOR=$env:CARGO_TERM_COLOR",
  "-e", "NPM_CONFIG_PROGRESS=$env:NPM_CONFIG_PROGRESS",
  "-e", "NPM_CONFIG_LOGLEVEL=$env:NPM_CONFIG_LOGLEVEL",
  "-e", "MARENGO_DEPLOY_VERBOSE=$env:MARENGO_DEPLOY_VERBOSE",
  "-e", "MARENGO_DEPLOY_START=$([DateTimeOffset]::new($DeployStart).ToUnixTimeSeconds())",
  "-e", "MARENGO_SSH_DIR=$env:MARENGO_SSH_DIR",
  "-v", "${DockerSshMount}:/home/marengo/.ssh:ro",
  "deploy-pi"
) + $deployArgs

& docker @composeArgs
$exit = $LASTEXITCODE
if ($exit -ne 0) {
  Write-Error "docker compose run failed (exit $exit)"
  exit $exit
}
