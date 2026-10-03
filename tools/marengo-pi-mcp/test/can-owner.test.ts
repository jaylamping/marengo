import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { canOwnerBranch, unlessCanOwned } from "../src/can-owner.js";

/** Run `script` under the remote preamble's strict mode with a stub `pgrep`. */
function runWithPgrep(script: string, pgrepOutput: string | null): string {
  const bin = mkdtempSync(path.join(tmpdir(), "can-owner-"));
  const stub =
    pgrepOutput === null ? "#!/bin/sh\nexit 1\n" : `#!/bin/sh\necho '${pgrepOutput}'\n`;
  writeFileSync(path.join(bin, "pgrep"), stub);
  chmodSync(path.join(bin, "pgrep"), 0o755);
  return execFileSync("bash", ["-c", `set -euo pipefail\n${script}`], {
    env: { ...process.env, PATH: `${bin}:${process.env.PATH ?? ""}` },
    encoding: "utf8",
  });
}

describe("CAN owner guard", () => {
  it("matches marengo-pi and motor-repl by exact process name", () => {
    assert.match(canOwnerBranch("free", "owned"), /pgrep -l -x 'marengo-pi\|motor-repl'/);
  });

  it("runs the command when no CAN owner is running", () => {
    const out = runWithPgrep(unlessCanOwned("echo motor-repl-ran"), null);
    assert.equal(out.trim(), "motor-repl-ran");
  });

  it("skips the command and names the owner while marengo-pi runs", () => {
    const out = runWithPgrep(unlessCanOwned("echo motor-repl-ran"), "4242 marengo-pi");
    // Only the skip notice (which names the command); the command itself never ran.
    assert.equal(out, "echo motor-repl-ran skipped: marengo-pi (pid 4242) owns CAN\n");
  });

  it("exposes owner name and pid to the owned branch", () => {
    const out = runWithPgrep(
      canOwnerBranch("echo free", 'echo "$CAN_OWNER_NAME/$CAN_OWNER_PID"'),
      "77 motor-repl",
    );
    assert.equal(out.trim(), "motor-repl/77");
  });
});
