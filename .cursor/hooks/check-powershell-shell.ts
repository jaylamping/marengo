/**
 * Cross-platform beforeShellExecution hook.
 * On Windows (native, not WSL): deny bash-isms aimed at PowerShell.
 * On macOS / Linux / WSL: allow (bash is the agent shell).
 *
 * Invoked via: node ".cursor/hooks/check-powershell-shell.js"
 * (built from this .ts — run `just mcp-build` after editing).
 */

function readStdin(): Promise<string> {
  return new Promise((resolve) => {
    const chunks: string[] = [];
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (c: string) => chunks.push(c));
    process.stdin.on("end", () => resolve(chunks.join("")));
    process.stdin.on("error", () => resolve(""));
    if (process.stdin.isTTY) resolve("");
  });
}

function isWindowsNative(): boolean {
  if (process.env.WSL_DISTRO_NAME) return false;
  return process.platform === "win32";
}

function isExplicitUnixShell(cmd: string): boolean {
  return (
    /^\s*(bash|sh|dash|zsh|wsl|wsl\.exe)\b/i.test(cmd) ||
    /\|\s*(sh|bash|dash|zsh)\b/i.test(cmd) ||
    /\b(?:bash|sh)\s+-c\b/i.test(cmd)
  );
}

function bashismIssues(cmd: string): string[] {
  const issues: string[] = [];
  if (/&&|\|\|/.test(cmd)) {
    issues.push(
      'bash &&/|| chaining - use "; if ($LASTEXITCODE -eq 0) { ... }"',
    );
  }
  if (/<<\s*['"]?[\w-]/.test(cmd)) {
    issues.push(
      "bash heredoc - pipe a PowerShell here-string into sh, or run via bash -c",
    );
  }
  if (/(?:^|[;&|]\s*)export\s+\w+=/m.test(cmd)) {
    issues.push('bash export - use $env:NAME = "value"');
  }
  if (/(?<![\w-])set\s+-e(uo)?\b/.test(cmd)) {
    issues.push("bash set -e - not valid in PowerShell");
  }
  if (/\$\(\s*(cat|dirname|basename|pwd|which|command)\b/.test(cmd)) {
    issues.push(
      "bash $(cmd) - use PowerShell equivalents (Get-Content, Split-Path, Get-Command)",
    );
  }
  if (/(?<![\w`$])if\s+\[\s/.test(cmd)) {
    issues.push("bash if [ ... ] - use PowerShell if ()");
  }
  if (/\[\[/.test(cmd)) {
    issues.push("bash [[ ... ]] - use PowerShell -match / -eq");
  }
  if (/(?:^|[;&|]\s*)source\s+/m.test(cmd)) {
    issues.push("bash source - use . ./file.ps1 or invoke via bash");
  }
  if (/(?<![\w/\\])\becho\s+-[neE]\b/.test(cmd)) {
    issues.push("bash echo -n/-e - use Write-Output / Write-Host");
  }
  return issues;
}

function emit(obj: Record<string, unknown>): void {
  process.stdout.write(JSON.stringify(obj));
}

interface ShellPayload {
  command?: unknown;
}

const raw = await readStdin();
let payload: ShellPayload = {};
try {
  payload = raw ? (JSON.parse(raw) as ShellPayload) : {};
} catch {
  emit({ permission: "allow" });
  process.exit(0);
}

const cmd = String(payload.command ?? "");
if (!cmd || !isWindowsNative() || isExplicitUnixShell(cmd)) {
  emit({ permission: "allow" });
  process.exit(0);
}

const issues = bashismIssues(cmd);
if (issues.length === 0) {
  emit({ permission: "allow" });
  process.exit(0);
}

const list = issues.map((i) => `- ${i}`).join("\n");
emit({
  permission: "deny",
  user_message: "Bash syntax blocked in PowerShell shell",
  agent_message: `Blocked: this Shell command uses bash-only syntax, but this Windows Cursor workspace runs PowerShell.

Issues:
${list}

Rewrite for PowerShell (see .cursor/rules/windows-shell.mdc), or wrap Unix syntax explicitly:
  bash -lc '...'
  @' ... '@ | sh

For Marengo software (cargo / just check / deploy), prefer the WSL clone at ~/code/marengo.`,
});
