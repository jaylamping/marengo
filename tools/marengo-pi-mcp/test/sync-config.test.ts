import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { MarengoPiConfig } from "../src/config.js";
import { registerAdminTools } from "../src/tools/admin.js";
import {
  benchUrdfInstallBody,
  benchUrdfStagingVerifyBody,
  directInstallRsyncLine,
} from "../src/tools/sync-config.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config/bringup/shoulder_pitch_right_only",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

describe("bench URDF sync", () => {
  it("registers a default right-bench URDF sync admin tool", async () => {
    const tools = registerAdminTools(cfg, async () => "(remote)");

    assert.ok(tools.pi_sync_bench_urdf);
    assert.deepEqual(tools.pi_sync_bench_urdf.inputSchema.parse({}), {
      assets: ["shoulder_pitch_right_only.urdf", "shoulder_pitch_weighted.urdf"],
      install_to_opt: true,
    });
  });

  it("installs selected URDFs into /opt without sudo when writable", () => {
    const script = benchUrdfInstallBody(
      cfg,
      ["shoulder_pitch_right_only.urdf", "shoulder_pitch_weighted.urdf"],
    );

    assert.match(script, /shoulder_pitch_right_only\.urdf/);
    assert.match(script, /shoulder_pitch_weighted\.urdf/);
    assert.match(script, /install -m 0644 "\$SRC\/\$asset" "\$DST\/\$asset"/);
    assert.match(script, /installed \$DST\/\$asset \(direct write\)/);
    assert.match(script, /grep -A3 "<inertial>"/);
    assert.doesNotMatch(script, /sudo install/);
  });

  it("verifies a single staged left bench URDF", () => {
    const script = benchUrdfStagingVerifyBody(
      cfg,
      ["shoulder_pitch_left_bare.urdf"],
    );

    assert.match(script, /shoulder_pitch_left_bare\.urdf/);
    assert.match(script, /grep -A3 "<inertial>"/);
    assert.doesNotMatch(script, /shoulder_pitch_right_only\.urdf/);
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
