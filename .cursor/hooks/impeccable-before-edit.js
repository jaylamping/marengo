/**
 * Cross-platform preToolUse hook for the vendored impeccable skill.
 * Runs the skill's engine launcher (`impeccable` on macOS / Linux / WSL,
 * `impeccable.cmd` on native Windows) with `hook-before-edit`, piping the
 * Cursor hook payload through stdin and its verdict back through stdout.
 * When the launcher is absent the hook allows the edit, as the upstream
 * POSIX-only hook command does.
 *
 * Invoked via: node ".cursor/hooks/impeccable-before-edit.js"
 * (built from this .ts — run `just mcp-build` after editing).
 */
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
function readStdin() {
    const { promise, resolve } = Promise.withResolvers();
    let data = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => (data += chunk));
    process.stdin.on("end", () => resolve(data));
    return promise;
}
function isWindowsNative() {
    if (process.env.WSL_DISTRO_NAME)
        return false;
    return process.platform === "win32";
}
const scripts = path.join(".cursor", "skills", "impeccable", "scripts");
const launcher = path.join(scripts, isWindowsNative() ? "impeccable.cmd" : "impeccable");
const payload = await readStdin();
if (existsSync(launcher)) {
    const result = spawnSync(launcher, ["hook-before-edit"], {
        input: payload,
        encoding: "utf8",
        shell: isWindowsNative(),
    });
    if (result.stdout)
        process.stdout.write(result.stdout);
    if (result.stderr)
        process.stderr.write(result.stderr);
    process.exitCode = result.status ?? 0;
}
