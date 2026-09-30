# Launcher for Cursor MCP on Windows.
# Cursor's spawn PATH often lacks mise `node` (ENOENT). Keep profile / SSH
# defaults in dist/launch.js (from src/launch.ts) — not in `.cursor/mcp.json`
# (env thrash auto-disables the server).
$ErrorActionPreference = 'Stop'
$Root = $PSScriptRoot

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
$Entry = Join-Path $Root 'dist\launch.js'
if (-not (Test-Path -LiteralPath $Entry)) {
    throw "run-mcp.ps1: missing $Entry — run ``just mcp-build`` first."
}
& $Node $Entry
exit $LASTEXITCODE
