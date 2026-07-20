# sessionStart: (1) ensure marengo-pi MCP enabled (2) inject shell/environment context.
$ErrorActionPreference = 'Continue'

$raw = [Console]::In.ReadToEnd()
$repo = $null
$composerMode = $null
try {
    $payload = $raw | ConvertFrom-Json
    if ($payload.workspace_roots -and $payload.workspace_roots.Count -gt 0) {
        $repo = [string]$payload.workspace_roots[0]
    }
    if ($payload.composer_mode) { $composerMode = [string]$payload.composer_mode }
} catch {
    # fall through
}
if (-not $repo) {
    $repo = if ($env:CURSOR_PROJECT_DIR) { $env:CURSOR_PROJECT_DIR } else { (Get-Location).Path }
}

$isWin = ($env:OS -eq 'Windows_NT') -and (-not $env:WSL_DISTRO_NAME)
$shellName = if ($isWin) { 'Windows PowerShell' } else { 'Unix/bash (or WSL)' }

# --- marengo-pi ensure ---
$mcpStatus = 'skipped'
$mcpDetail = 'ensure script missing'
$script = Join-Path $repo 'scripts\ensure-marengo-pi-mcp-enabled.py'
if (Test-Path -LiteralPath $script) {
    $py = $null
    foreach ($name in @('py', 'python3', 'python')) {
        $cmd = Get-Command $name -ErrorAction SilentlyContinue
        if ($cmd) { $py = $cmd.Source; break }
    }
    if ($py) {
        $pyArgs = @()
        if ((Split-Path -Leaf $py) -eq 'py.exe') { $pyArgs += '-3' }
        $pyArgs += @($script, '--write', '--best-effort', '--repo', $repo)
        $out = & $py @pyArgs 2>&1 | Out-String
        if ($LASTEXITCODE -eq 0) {
            $mcpStatus = 'ok'
            $mcpDetail = if ($out -match 'No changes needed') { 'already enabled' }
                elseif ($out -match 'after:') { 'scrubbed/approved' }
                else { 'ran' }
        } else {
            $mcpStatus = 'failed'
            $mcpDetail = "exit $LASTEXITCODE"
        }
    } else {
        $mcpDetail = 'python not on PATH'
    }
}

$repoNorm = $repo -replace '\\', '/'
$onWindowsClone = $isWin -and ($repoNorm -match '(?i)^[A-Za-z]:/code/marengo')
$onWslMount = $repoNorm -match '(?i)^/mnt/[a-z]/'
$softwareHint = if ($onWindowsClone -or $onWslMount) {
    'Software work (cargo, just check, Pi deploy): prefer WSL clone at ~/code/marengo (ext4). This Windows/mount path is for CAD / SolidWorks MCP.'
} elseif ($isWin) {
    'Shell is PowerShell — never emit bash &&/||/heredocs unless wrapped in bash/sh. beforeShellExecution will deny bash-isms.'
} else {
    'Unix shell OK for bash syntax. Keep Marengo software on the WSL/Linux clone when possible.'
}

$ctx = @"
## Marengo session environment
- Shell host: $shellName
- Workspace: $repo
- Composer mode: $(if ($composerMode) { $composerMode } else { 'unknown' })
- marengo-pi MCP ensure: $mcpStatus ($mcpDetail)
- $softwareHint

PowerShell sequencing (mandatory on Windows): use ``; if (`$LASTEXITCODE -eq 0) { ... }`` — not ``&&`` / ``||``.
Git commit heredocs: pipe a PowerShell here-string into ``sh``. See ``.cursor/rules/windows-shell.mdc``.
If marengo-pi tools are missing: enable/restart MCP, or quit Cursor and run ``just mcp-ensure-enabled --write``.
"@.Trim()

$envMap = [ordered]@{
    MARENGO_CURSOR_SHELL = $(if ($isWin) { 'powershell' } else { 'unix' })
    MARENGO_WORKSPACE_ROOT = $repo
}

[ordered]@{
    additional_context = $ctx
    env = $envMap
} | ConvertTo-Json -Compress -Depth 4
