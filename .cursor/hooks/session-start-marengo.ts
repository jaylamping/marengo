/**
 * Cross-platform sessionStart: inject shell/host context.
 *
 * It never re-enables the marengo-pi MCP server: a user may disable that
 * motor-driving server as a kill switch (TOOL-09). Re-enabling it is the
 * explicit `just mcp-ensure-enabled --write`.
 *
 * Invoked via: node ".cursor/hooks/session-start-marengo.js"
 * (built from this .ts — run `just mcp-build` after editing).
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

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

/**
 * On non-Windows hosts, remove Windows-only MCP servers (SolidWorks needs the
 * Windows SolidWorks API) from .cursor/mcp.json. The committed file keeps the
 * entry so Windows checkouts retain it; this re-applies the local strip after
 * any git operation (checkout/restore/pull) re-adds it.
 */
function stripWindowsOnlyMcpServers(): string {
  if (isWindowsNative()) return "skipped (windows)";
  const file = path.join(repo, ".cursor", "mcp.json");
  if (!fs.existsSync(file)) return "no mcp.json";
  try {
    const before = fs.readFileSync(file, "utf8");
    const key = before.search(/[ \t]*"solidworks"[ \t]*:/);
    if (key === -1) return "up to date";
    const open = before.indexOf("{", before.indexOf(":", key));
    if (open === -1) return "failed: no object after solidworks key";
    // Brace-counting scan: a lazy regex stops at the first nested closer.
    let depth = 0;
    let close = -1;
    for (let i = open; i < before.length; i++) {
      const c = before[i];
      if (c === "{") depth++;
      else if (c === "}") {
        depth--;
        if (depth === 0) {
          close = i;
          break;
        }
      }
    }
    if (close === -1) return "failed: unbalanced solidworks object";
    let end = close + 1;
    if (before[end] === ",") end++;
    if (before[end] === "\r") end++;
    if (before[end] === "\n") end++;
    const after = before.slice(0, key) + before.slice(end);
    JSON.parse(after); // never write an invalid config
    fs.writeFileSync(file, after);
    return "stripped solidworks";
  } catch (e) {
    return `failed: ${e instanceof Error ? e.message : String(e)}`;
  }
}

interface SessionPayload {
  workspace_roots?: unknown;
  composer_mode?: string;
}

const raw = await readStdin();
let payload: SessionPayload = {};
try {
  payload = raw ? (JSON.parse(raw) as SessionPayload) : {};
} catch {
  /* ignore */
}

const repo =
  (Array.isArray(payload.workspace_roots) &&
    typeof payload.workspace_roots[0] === "string" &&
    payload.workspace_roots[0]) ||
  process.env.CURSOR_PROJECT_DIR ||
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

const composerMode = payload.composer_mode || "unknown";
const win = isWindowsNative();
const shellName = win ? "Windows PowerShell" : "Unix/bash (macOS/Linux)";

const mcpConfigNote = stripWindowsOnlyMcpServers();

let softwareHint: string;
if (win) {
  softwareHint =
    "Software and local CAD live in the host checkout at J:\\code\\marengo. Use Docker for full Linux workspace checks. Shell is PowerShell; wrap Unix recipes explicitly in Git for Windows sh when needed.";
} else {
  softwareHint =
    "Use the host checkout on macOS/Linux, with Docker for Linux runtime checks. Ubuntu is not a separate required software home.";
}

const ctx = `## Marengo session environment
- Shell host: ${shellName}
- Workspace: ${repo}
- Composer mode: ${composerMode}
- mcp.json (host): ${mcpConfigNote}
- ${softwareHint}

PowerShell sequencing (mandatory on Windows): use \`; if ($LASTEXITCODE -eq 0) { ... }\` — not \`&&\` / \`||\`.
Git commit heredocs on Windows: pipe a PowerShell here-string into \`sh\`. See \`.cursor/rules/windows-shell.mdc\`.
If marengo-pi tools are missing, ask the user before re-enabling the MCP server: they may have disabled it on purpose. Re-enable in Cursor's MCP settings, or quit Cursor and run \`just mcp-ensure-enabled --write\`.`;

process.stdout.write(
  JSON.stringify({
    additional_context: ctx,
    env: {
      MARENGO_CURSOR_SHELL: win ? "powershell" : "unix",
      MARENGO_WORKSPACE_ROOT: repo,
    },
  }),
);
