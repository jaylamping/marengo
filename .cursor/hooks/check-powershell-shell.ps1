# beforeShellExecution: block bash-only syntax when Cursor's shell is PowerShell.
# Fail-open on parse errors. Stdin = hook JSON; stdout = permission JSON.
$ErrorActionPreference = 'Continue'

function Write-Permission([string]$permission, [string]$agentMessage = '', [string]$userMessage = '') {
    $o = [ordered]@{ permission = $permission }
    if ($agentMessage) { $o.agent_message = $agentMessage }
    if ($userMessage) { $o.user_message = $userMessage }
    $o | ConvertTo-Json -Compress
}

function Test-ExplicitUnixShell([string]$cmd) {
    if ($cmd -match '(?i)^\s*(bash|sh|dash|zsh|wsl|wsl\.exe)\b') { return $true }
    # Approved pattern from windows-shell.mdc: here-string piped into sh/bash
    if ($cmd -match '(?i)\|\s*(sh|bash|dash|zsh)\b') { return $true }
    if ($cmd -match '(?i)\b(?:bash|sh)\s+-c\b') { return $true }
    return $false
}

function Get-BashismIssues([string]$cmd) {
    $issues = [System.Collections.Generic.List[string]]::new()
    if ($cmd -match '&&|\|\|') {
        $issues.Add('bash &&/|| chaining - use "; if ($LASTEXITCODE -eq 0) { ... }"')
    }
    if ($cmd -match '<<\s*[''"]?[\w-]+') {
        $issues.Add('bash heredoc - pipe a PowerShell here-string into sh, or run via bash -c')
    }
    if ($cmd -match '(?m)(?:^|[;&|]\s*)export\s+\w+=') {
        $issues.Add('bash export - use $env:NAME = "value"')
    }
    if ($cmd -match '(?<![\w-])set\s+-e(uo)?\b') {
        $issues.Add('bash set -e - not valid in PowerShell')
    }
    if ($cmd -match '\$\(\s*(cat|dirname|basename|pwd|which|command)\b') {
        $issues.Add('bash $(cmd) - use PowerShell equivalents (Get-Content, Split-Path, Get-Command)')
    }
    if ($cmd -match '(?<![\w`$])if\s+\[\s') {
        $issues.Add('bash if [ ... ] - use PowerShell if ()')
    }
    if ($cmd -match '\[\[') {
        $issues.Add('bash [[ ... ]] - use PowerShell -match / -eq')
    }
    if ($cmd -match '(?m)(?:^|[;&|]\s*)source\s+') {
        $issues.Add('bash source - use . ./file.ps1 or invoke via bash')
    }
    if ($cmd -match '(?<![\w/\\])\becho\s+-[neE]\b') {
        $issues.Add('bash echo -n/-e - use Write-Output / Write-Host')
    }
    return $issues
}

function Test-WindowsNativeShell {
    if ($env:WSL_DISTRO_NAME) { return $false }
    if ($env:OS -eq 'Windows_NT') { return $true }
    if ($IsWindows -eq $true) { return $true }
    return $false
}

try {
    $raw = [Console]::In.ReadToEnd()
    $payload = $null
    try { $payload = $raw | ConvertFrom-Json } catch { }

    $cmd = if ($payload -and $payload.command) { [string]$payload.command } else { '' }
    $cwd = if ($payload -and $payload.cwd) { [string]$payload.cwd } else { '' }

    if (-not $cmd) {
        Write-Permission 'allow'
        exit 0
    }

    # Only enforce on native Windows PowerShell hosts (not WSL/Linux bash agents).
    if (-not (Test-WindowsNativeShell)) {
        Write-Permission 'allow'
        exit 0
    }

    if (Test-ExplicitUnixShell $cmd) {
        Write-Permission 'allow'
        exit 0
    }

    $issues = Get-BashismIssues $cmd
    if ($issues.Count -gt 0) {
        $list = ($issues | ForEach-Object { "- $_" }) -join "`n"
        $agent = @"
Blocked: this Shell command uses bash-only syntax, but this Windows Cursor workspace runs PowerShell.

Issues:
$list

Rewrite for PowerShell (see .cursor/rules/windows-shell.mdc), or wrap Unix syntax explicitly:
  bash -lc '...'
  @' ... '@ | sh

For Marengo software (cargo / just check / deploy), prefer the WSL clone at ~/code/marengo.
"@.Trim()
        Write-Permission 'deny' $agent 'Bash syntax blocked in PowerShell shell'
        exit 0
    }

    # Soft nudge only via allow — heavy software gates live in sessionStart context.
    Write-Permission 'allow'
    exit 0
} catch {
    Write-Permission 'allow'
    exit 0
}
