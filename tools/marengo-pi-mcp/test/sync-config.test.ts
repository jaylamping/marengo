import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { MarengoPiConfig } from "../src/config.js";
import { registerAdminTools } from "../src/tools/admin.js";
import {
  benchUrdfInstallBody,
  benchUrdfStagingVerifyBody,
  directInstallRsyncLine,
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
    const script = benchUrdfStagingVerifyBody(
      cfg,
      ["shoulder_pitch_left_bare.urdf"],
    );

    assert.match(script, /shoulder_pitch_left_bare\.urdf/);
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
  it("avoids chmod/chown metadata writes during direct /opt installs", async () => {
    assert.equal(
      directInstallRsyncLine,
      'rsync -r --no-owner --no-group --no-perms --omit-dir-times "$SRC/" "$DST/"',
    );
    assert.doesNotMatch(directInstallRsyncLine, /rsync -a/);
  });
});
