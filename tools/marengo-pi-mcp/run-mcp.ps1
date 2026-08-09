# Launcher for Cursor MCP on Windows.
# Cursor's spawn PATH often lacks mise `node` (ENOENT). Keep profile / SSH
# defaults HERE — not in `.cursor/mcp.json` (env thrash auto-disables the server).
$ErrorActionPreference = 'Stop'
$Root = $PSScriptRoot

if (-not $env:MARENGO_PI_HOST) { $env:MARENGO_PI_HOST = 'joey-robot.tail0b414.ts.net' }
if (-not $env:MARENGO_PI_USER) { $env:MARENGO_PI_USER = 'joey' }
if (-not $env:MARENGO_PI_ROOT) { $env:MARENGO_PI_ROOT = '/opt/marengo' }
if (-not $env:MARENGO_CONFIG_DIR) {
    $env:MARENGO_CONFIG_DIR = '/opt/marengo/config'
}
if (-not $env:MARENGO_BENCH_PROFILE) { $env:MARENGO_BENCH_PROFILE = 'elbow_attached' }

if (-not $env:SSH_IDENTITY_FILE) {
    $home = $env:USERPROFILE
    if (-not $home) { $home = $env:HOME }
    foreach ($candidate in @(
            (Join-Path $home '.ssh\id_ed25519_marengo'),
            (Join-Path $home '.ssh\id_ed25519')
        )) {
        if (Test-Path -LiteralPath $candidate) {
            $env:SSH_IDENTITY_FILE = $candidate
            break
        }
    }
}

function Resolve-MarengoNode {
    if ($env:MARENGO_MCP_NODE -and (Test-Path -LiteralPath $env:MARENGO_MCP_NODE)) {
        return $env:MARENGO_MCP_NODE
    }
    $mise = Get-Command mise -ErrorAction SilentlyContinue
    if ($mise) {
        try {
            $miseNode = (& mise which node 2>$null)
            if ($miseNode -and (Test-Path -LiteralPath $miseNode.Trim())) {
                return $miseNode.Trim()
            }
        } catch {
            # fall through
        }
    }
    $home = $env:USERPROFILE
    if (-not $home) { $home = $env:HOME }
    foreach ($candidate in @(
            (Join-Path $home '.local\share\mise\shims\node.exe'),
            (Join-Path $home 'AppData\Local\mise\installs\node\24.16.0\node.exe'),
            (Join-Path $home '.local\share\mise\installs\node\24.16.0\bin\node'),
            'C:\Program Files\nodejs\node.exe'
        )) {
        if ($candidate -and (Test-Path -LiteralPath $candidate)) {
            return $candidate
        }
    }
    $onPath = Get-Command node -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }
    throw 'run-mcp.ps1: node not found. Install Node 24 (mise) or set MARENGO_MCP_NODE.'
}

$Node = Resolve-MarengoNode
$env:PATH = "$(Split-Path -Parent $Node);$env:PATH"
$Entry = Join-Path $Root 'dist\index.js'
if (-not (Test-Path -LiteralPath $Entry)) {
    throw "run-mcp.ps1: missing $Entry — run ``just mcp-build`` first."
}
& $Node $Entry
exit $LASTEXITCODE
