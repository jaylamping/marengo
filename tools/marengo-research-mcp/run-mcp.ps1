# Marengo research MCP launcher — injects GITHUB_TOKEN from `gh auth token` when available.
$ErrorActionPreference = 'Stop'
$Root = $PSScriptRoot
$RepoRoot = (Resolve-Path (Join-Path $Root '../..')).Path

if (-not $env:MARENGO_RESEARCH_CACHE_DIR) {
    $env:MARENGO_RESEARCH_CACHE_DIR = Join-Path $RepoRoot '.marengo-research'
}
if (-not $env:MARENGO_WORKSPACE) {
    $env:MARENGO_WORKSPACE = $RepoRoot
}

if (-not $env:GITHUB_TOKEN) {
    $gh = Get-Command gh -ErrorAction SilentlyContinue
    if ($gh) {
        try {
            $token = (& gh auth token 2>$null)
            if ($token) {
                $env:GITHUB_TOKEN = $token.Trim()
            }
        } catch {
            # GitHub search works without token at lower rate limits
        }
    }
}

Set-Location $Root
& uv run python -m marengo_research_mcp.server
