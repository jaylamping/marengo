import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { canOwnerBranch, soleCanOwnerShell, unlessCanOwned } from "../src/can-owner.js";
import { INSTALLED_RESTART_HELPER } from "../src/tools/restart-marengo-pi.js";

interface PiStub {
  /** `pgrep -l` line for a CAN owner, or null when the bus is free. */
  owner?: string | null;
  /** Owner disappears once the restart helper runs `stop` (service-owned marengo-pi). */
  ownerStopsWithUnit?: boolean;
  unitState?: string;
}

/**
 * Run `script` under the remote preamble's strict mode with stubbed Pi commands.
 * `calls` records sudo/pkill invocations in order.
 */
function runOnStubPi(script: string, stub: PiStub = {}) {
  const dir = mkdtempSync(path.join(tmpdir(), "can-owner-"));
  const bin = path.join(dir, "bin");
  const calls = path.join(dir, "calls");
  const ownerFile = path.join(dir, "owner");
  writeFileSync(calls, "");
  writeFileSync(ownerFile, stub.owner ?? "");
  const stubs: Record<string, string> = {
    pgrep: `[ -s "${ownerFile}" ] || exit 1\n[ "$1" = -l ] && cat "${ownerFile}"\nexit 0`,
    pkill: `echo "pkill $*" >> "${calls}"\nexit 1`,
    sudo: [
      `echo "sudo $*" >> "${calls}"`,
      stub.ownerStopsWithUnit ? `[ "$3" = stop ] && : > "${ownerFile}"` : ":",
      "exit 0",
    ].join("\n"),
    systemctl: `echo "${stub.unitState ?? "inactive"}"\n[ "${stub.unitState}" = active ]`,
    sleep: "exit 0",
  };
  mkdirSync(bin);
  for (const [name, body] of Object.entries(stubs)) {
    writeFileSync(path.join(bin, name), `#!/bin/sh\n${body}\n`);
    chmodSync(path.join(bin, name), 0o755);
  }
  const r = spawnSync("bash", ["-c", `set -euo pipefail\n${script}`], {
    env: { ...process.env, PATH: `${bin}:${process.env.PATH ?? ""}` },
    encoding: "utf8",
  });
  return { stdout: r.stdout, stderr: r.stderr, status: r.status, calls: readFileSync(calls, "utf8") };
}

describe("CAN owner guard", () => {
  it("matches marengo-pi and motor-repl by exact process name", () => {
    assert.match(canOwnerBranch("free", "owned"), /pgrep -l -x 'marengo-pi\|motor-repl'/);
  });

  it("runs the command when no CAN owner is running", () => {
    const r = runOnStubPi(unlessCanOwned("echo motor-repl-ran"));
    assert.equal(r.stdout, "motor-repl-ran\n");
  });

  it("skips the command and names the owner while marengo-pi runs", () => {
    const r = runOnStubPi(unlessCanOwned("echo motor-repl-ran"), { owner: "4242 marengo-pi" });
    // Only the skip notice (which names the command); the command itself never ran.
    assert.equal(r.stdout, "echo motor-repl-ran skipped: marengo-pi (pid 4242) owns CAN\n");
  });

  it("exposes owner name and pid to the owned branch", () => {
    const r = runOnStubPi(
      canOwnerBranch("echo free", 'echo "$CAN_OWNER_NAME/$CAN_OWNER_PID"'),
      { owner: "77 motor-repl" },
    );
    assert.equal(r.stdout, "motor-repl/77\n");
  });
});

describe("sole CAN owner session", () => {
  const helper = INSTALLED_RESTART_HELPER;

  it("stops an active unit via the restart helper, runs, then restores it", () => {
    const r = runOnStubPi(soleCanOwnerShell("echo BODY"), {
      owner: "900 marengo-pi",
      ownerStopsWithUnit: true,
      unitState: "active",
    });
    assert.equal(r.status, 0, r.stderr);
    // The restore waits for a settled bus before marengo-pi.service starts on it.
    assert.match(
      r.stdout,
      /restore after session: true\n[\s\S]*BODY\ncan settle: ok [^\n]*\n=== restoring marengo-pi\.service/,
    );
    assert.deepEqual(r.calls.trim().split("\n"), [
      `sudo -n ${helper} stop`,
      "pkill -x marengo-pi",
      "pkill -x marengo-pi",
      `sudo -n ${helper} restart`,
    ]);
  });

  it("refuses when an owner survives the stop, and still restores the unit", () => {
    const r = runOnStubPi(soleCanOwnerShell("echo BODY"), {
      owner: "55 marengo-pi",
      unitState: "active",
    });
    assert.equal(r.status, 1);
    assert.doesNotMatch(r.stdout, /BODY/);
    assert.match(r.stderr, /marengo-pi \(pid 55\) still owns CAN; refusing/);
    assert.match(r.calls, new RegExp(`sudo -n ${helper} restart`));
  });

  it("preserves the body exit status and restores exactly once", () => {
    // The pipeline subshell must not inherit the EXIT trap.
    const r = runOnStubPi(soleCanOwnerShell("{ exit 3; } | cat || true\nexit 7"), {
      unitState: "activating",
    });
    assert.equal(r.status, 7);
    assert.equal(r.calls.match(/ restart$/gm)?.length, 1);
  });

  it("leaves an inactive unit stopped", () => {
    const r = runOnStubPi(soleCanOwnerShell("echo BODY"), { unitState: "inactive" });
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /restore after session: false/);
    assert.doesNotMatch(r.calls, / restart$/m);
  });
});
