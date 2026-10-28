import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { defaultLocalRoot } from "../src/config.js";
import type { MarengoPiConfig } from "../src/config.js";
import { registerAdminTools } from "../src/tools/admin.js";
import {
  benchUrdfInstallBody,
  benchUrdfStagingVerifyBody,
  benchConfigInstallBody,
  directInstallRsyncLine,
  preserveTaughtLimitsShell,
  remoteStepFailed,
} from "../src/tools/sync-config.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

describe("bench URDF sync", () => {
  it("registers a default master URDF sync admin tool", async () => {
    const tools = registerAdminTools(cfg, async () => "(remote)");

    assert.ok(tools.pi_sync_bench_urdf);
    assert.deepEqual(tools.pi_sync_bench_urdf.inputSchema.parse({}), {
      assets: ["marengo.urdf"],
      install_to_opt: true,
    });
  });

  it("installs selected URDFs into /opt without sudo when writable", () => {
    const script = benchUrdfInstallBody(cfg, ["marengo.urdf"]);

    assert.match(script, /marengo\.urdf/);
    assert.match(script, /install -m 0644 "\$SRC\/\$asset" "\$DST\/\$asset"/);
    assert.match(script, /installed \$DST\/\$asset \(direct write\)/);
    assert.match(script, /grep -A3 "<inertial>"/);
    assert.doesNotMatch(script, /sudo install/);
  });

  it("fails closed when /opt is not writable (no stale inertial dump)", () => {
    const script = benchUrdfInstallBody(cfg, ["marengo.urdf"]);

    assert.match(script, /pi_install_staging/);
    assert.match(script, /exit 1/);
    const elseIdx = script.indexOf("cannot write");
    assert.ok(elseIdx > 0);
    const afterElse = script.slice(elseIdx);
    assert.doesNotMatch(afterElse, /grep -A3 "<inertial>"/);
  });

  it("verifies a single staged archive URDF", () => {
    const script = benchUrdfStagingVerifyBody(cfg, [
      "archive/seed-shoulder_pitch_left_bare/contributor.urdf",
    ]);

    assert.match(script, /seed-shoulder_pitch_left_bare\/contributor\.urdf/);
    assert.match(script, /grep -A3 "<inertial>"/);
    assert.doesNotMatch(script, /marengo\.urdf/);
  });

  it("detects non-zero remote exit markers for MCP isError", () => {
    assert.equal(remoteStepFailed("ok\n[exit 0]"), false);
    assert.equal(remoteStepFailed("fail\n[exit 1]"), true);
    assert.equal(remoteStepFailed("timeout\n[exit 124]"), true);
    assert.equal(remoteStepFailed("plain output"), false);
  });
});

describe("bench config sync", () => {
  it("copies config without chmod/chown metadata writes or the runtime profile lock", async () => {
    assert.doesNotMatch(directInstallRsyncLine, /rsync -a/);
    for (const flag of ["--no-owner", "--no-group", "--no-perms", "--omit-dir-times"]) {
      assert.ok(directInstallRsyncLine.includes(flag), `missing ${flag}`);
    }
    assert.match(directInstallRsyncLine, /--exclude="\.marengo-profile\.lock"/);
  });
});

describe("direct /opt writes keep taught limits (TOOL-08)", () => {
  const motors = (lower: number, upper: number) =>
    [
      "motors:",
      "  - joint: right_elbow_pitch",
      "    bench:",
      `      position_lower_rad: ${lower}`,
      `      position_upper_rad: ${upper}`,
      "",
    ].join("\n");

  function tree() {
    const root = mkdtempSync(path.join(tmpdir(), "taught-"));
    const opt = path.join(root, "opt");
    const staged = path.join(root, "staged");
    for (const dir of [opt, staged]) mkdirSync(path.join(dir, "config"), { recursive: true });
    // Taught on the Pi (Set Limits Apply) vs a wider envelope from git.
    writeFileSync(path.join(opt, "config", "motors.yaml"), motors(-0.2, 1.1));
    writeFileSync(path.join(staged, "config", "motors.yaml"), motors(-1.5, 2.5));
    return { opt, staged };
  }

  function run(opt: string, staged: string, preserveScript: string) {
    const shell = preserveTaughtLimitsShell(opt, [`cp "${staged}/config/motors.yaml" "$OPT/config/motors.yaml"`], preserveScript);
    return spawnSync("bash", ["-c", `set -euo pipefail\n${shell}`], { encoding: "utf8" });
  }

  it("a config sync restores the taught bounds after writing git's file", () => {
    const { opt, staged } = tree();
    const r = run(opt, staged, path.join(defaultLocalRoot(), "scripts", "preserve-taught-limits.py"));
    assert.equal(r.status, 0, r.stderr);
    const installed = readFileSync(path.join(opt, "config", "motors.yaml"), "utf8");
    assert.match(installed, /position_lower_rad: -0\.2\b/);
    assert.match(installed, /position_upper_rad: 1\.1\b/);
  });

  it("puts the previous files back and fails when the merge cannot run", () => {
    const { opt, staged } = tree();
    const r = run(opt, staged, "/nonexistent/preserve-taught-limits.py");
    assert.equal(r.status, 1);
    assert.match(r.stderr, /putting the previous limit files back/);
    assert.equal(readFileSync(path.join(opt, "config", "motors.yaml"), "utf8"), motors(-0.2, 1.1));
  });

  it("both sync tools wrap their /opt write in the merge", () => {
    for (const body of [benchConfigInstallBody(cfg), benchUrdfInstallBody(cfg, ["marengo.urdf"])]) {
      const backup = body.indexOf('_taught="$(mktemp -d)"');
      const write = body.search(/rsync -r|install -m 0644/);
      const merge = body.indexOf("preserve-taught-limits.py");
      assert.ok(backup > 0 && backup < write && write < merge, "backup, write, then merge");
    }
  });
});
