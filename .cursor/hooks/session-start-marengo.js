/**
 * Cross-platform sessionStart: ensure marengo-pi MCP + inject shell/host context.
 *
 * Invoked via: node ".cursor/hooks/session-start-marengo.js"
 * (built from this .ts — run `just mcp-build` after editing).
 */
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
function readStdin() {
    return new Promise((resolve) => {
        const chunks = [];
        process.stdin.setEncoding("utf8");
        process.stdin.on("data", (c) => chunks.push(c));
        process.stdin.on("end", () => resolve(chunks.join("")));
        process.stdin.on("error", () => resolve(""));
        if (process.stdin.isTTY)
            resolve("");
    });
}
function isWindowsNative() {
    if (process.env.WSL_DISTRO_NAME)
        return false;
    return process.platform === "win32";
}
function findPython() {
    const candidates = process.platform === "win32"
        ? [
            ["py", ["-3"]],
            ["python3", []],
            ["python", []],
        ]
        : [
            ["python3", []],
            ["python", []],
        ];
    for (const [bin, prefix] of candidates) {
        const probe = spawnSync(bin, [...prefix, "-c", "print(1)"], {
            encoding: "utf8",
            windowsHide: true,
        });
        if (probe.status === 0)
            return { bin, prefix };
    }
    return null;
}
const raw = await readStdin();
let payload = {};
try {
    payload = raw ? JSON.parse(raw) : {};
}
catch {
    /* ignore */
}
const repo = (Array.isArray(payload.workspace_roots) &&
    typeof payload.workspace_roots[0] === "string" &&
    payload.workspace_roots[0]) ||
    process.env.CURSOR_PROJECT_DIR ||
    path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const composerMode = payload.composer_mode || "unknown";
const win = isWindowsNative();
const shellName = win ? "Windows PowerShell" : "Unix/bash (macOS/Linux/WSL)";
let mcpStatus = "skipped";
let mcpDetail = "ensure script missing";
const ensureScript = path.join(repo, "scripts", "ensure-marengo-pi-mcp-enabled.py");
if (fs.existsSync(ensureScript)) {
    const py = findPython();
    if (!py) {
        mcpDetail = "python not on PATH";
    }
    else {
        const result = spawnSync(py.bin, [
            ...py.prefix,
            ensureScript,
            "--write",
            "--best-effort",
            "--repo",
            repo,
        ], { encoding: "utf8", windowsHide: true });
        if (result.status === 0) {
            mcpStatus = "ok";
            const out = `${result.stdout || ""}${result.stderr || ""}`;
            if (/No changes needed/.test(out))
                mcpDetail = "already enabled";
            else if (/after:/.test(out))
                mcpDetail = "scrubbed/approved";
            else
                mcpDetail = "ran";
        }
        else {
            mcpStatus = "failed";
            mcpDetail = `exit ${result.status}`;
        }
    }
}
const repoNorm = String(repo).replace(/\\/g, "/");
const onWindowsClone = win && /^[A-Za-z]:\/code\/marengo/i.test(repoNorm);
const onWslMount = /^\/mnt\/[a-z]\//i.test(repoNorm);
let softwareHint;
if (onWindowsClone || onWslMount) {
    softwareHint =
        "Software work (cargo, just check, Pi deploy): prefer WSL clone at ~/code/marengo (ext4). This Windows/mount path is for CAD / SolidWorks MCP.";
}
else if (win) {
    softwareHint =
        "Shell is PowerShell — never emit bash &&/||/heredocs unless wrapped in bash/sh. beforeShellExecution will deny bash-isms.";
}
else {
    softwareHint =
        "Unix shell OK for bash syntax. Mac/Linux/WSL: keep software on the native clone (not /mnt/c).";
}
const ctx = `## Marengo session environment
- Shell host: ${shellName}
- Workspace: ${repo}
- Composer mode: ${composerMode}
- marengo-pi MCP ensure: ${mcpStatus} (${mcpDetail})
- ${softwareHint}

PowerShell sequencing (mandatory on Windows): use \`; if ($LASTEXITCODE -eq 0) { ... }\` — not \`&&\` / \`||\`.
Git commit heredocs on Windows: pipe a PowerShell here-string into \`sh\`. See \`.cursor/rules/windows-shell.mdc\`.
If marengo-pi tools are missing: enable/restart MCP, or quit Cursor and run \`just mcp-ensure-enabled --write\`.`;
process.stdout.write(JSON.stringify({
    additional_context: ctx,
    env: {
        MARENGO_CURSOR_SHELL: win ? "powershell" : "unix",
        MARENGO_WORKSPACE_ROOT: repo,
    },
}));
