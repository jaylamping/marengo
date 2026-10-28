import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
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
  runSyncBenchUrdfAssets,
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

  it("keeps nested archive URDF paths when staging them (TOOL-21)", async () => {
    const asset = "archive/seed-shoulder_pitch_left_bare/contributor.urdf" as const;
    const localDir = path.join(cfg.localRoot, "assets", "urdf");
    for (const rsyncExit of [0, 1]) {
      const calls: Array<{ cmd: string; args: string[]; cwd?: string }> = [];
      const remote: string[] = [];
      await runSyncBenchUrdfAssets(
        cfg,
        async (body) => {
          remote.push(body);
          return "ok";
        },
        { assets: ["marengo.urdf", asset], install_to_opt: false },
        async (cmd, args, opts = {}) => {
          calls.push({ cmd, args, cwd: opts.cwd });
          return { stdout: "", stderr: "", exitCode: cmd === "rsync" ? rsyncExit : 0 };
        },
      );
      const [rsync, ...scps] = calls;
      // rsync -R from assets/urdf recreates archive/<seed>/ under the staging assets/urdf.
      assert.deepEqual(rsync, {
        cmd: "rsync",
        args: ["-avR", "marengo.urdf", asset, "joey@marengo.local:~/marengo/assets/urdf/"],
        cwd: localDir,
      });
      if (rsyncExit === 0) {
        assert.equal(scps.length, 0);
        continue;
      }
      // scp fallback: the nested directory exists first, then each file goes to its own path.
      assert.match(remote[0], /mkdir -p "\$HOME\/marengo\/assets\/urdf\/archive\/seed-shoulder_pitch_left_bare"/);
      assert.deepEqual(
        scps.map((c) => c.args),
        [
          [path.join(localDir, "marengo.urdf"), "joey@marengo.local:~/marengo/assets/urdf/marengo.urdf"],
          [path.join(localDir, asset), `joey@marengo.local:~/marengo/assets/urdf/${asset}`],
        ],
      );
    }
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

  // Rust profile writers (Set Limits write-behind) hold an exclusive flock(2) on this file.
  const lockFile = (opt: string) => path.join(opt, "config", ".marengo-profile.lock");
  const flockAvailable = spawnSync("sh", ["-c", "command -v flock"]).status === 0;

  it("holds the profile lock from the backup through the merge", { skip: !flockAvailable && "needs flock" }, () => {
    const { opt, staged } = tree();
    const seen = path.join(opt, "lock-during-write");
    const shell = preserveTaughtLimitsShell(
      opt,
      [
        `cp "${staged}/config/motors.yaml" "$OPT/config/motors.yaml"`,
        // A second writer (a new open file description) cannot take the lock meanwhile.
        `if flock -n "$OPT/config/.marengo-profile.lock" true; then echo free > ${seen}; else echo held > ${seen}; fi`,
      ],
      path.join(defaultLocalRoot(), "scripts", "preserve-taught-limits.py"),
    );
    const r = spawnSync("bash", ["-c", `set -euo pipefail\n${shell}`], { encoding: "utf8" });
    assert.equal(r.status, 0, r.stderr);
    assert.equal(readFileSync(seen, "utf8").trim(), "held");
    assert.equal(spawnSync("flock", ["-n", lockFile(opt), "true"]).status, 0, "released afterwards");
  });

  it("fails closed, writing nothing, while a profile writer holds the lock", { skip: !flockAvailable && "needs flock" }, async () => {
    const { opt, staged } = tree();
    writeFileSync(lockFile(opt), "");
    const holder = spawn("flock", ["-x", lockFile(opt), "sleep", "20"], { stdio: "ignore" });
    try {
      for (let i = 0; i < 50 && spawnSync("flock", ["-n", lockFile(opt), "true"]).status === 0; i++) {
        await new Promise((r) => setTimeout(r, 50));
      }
      const shell = preserveTaughtLimitsShell(
        opt,
        [`cp "${staged}/config/motors.yaml" "$OPT/config/motors.yaml"`],
        path.join(defaultLocalRoot(), "scripts", "preserve-taught-limits.py"),
        1,
      );
      const r = spawnSync("bash", ["-c", `set -euo pipefail\n${shell}`], { encoding: "utf8", timeout: 15_000 });
      assert.equal(r.status, 1, r.stdout);
      assert.match(r.stderr, /profile lock/);
      assert.equal(readFileSync(path.join(opt, "config", "motors.yaml"), "utf8"), motors(-0.2, 1.1));
    } finally {
      holder.kill("SIGKILL");
    }
  });

  it("takes the lock before the backup and releases it after the merge or rollback", () => {
    const shell = preserveTaughtLimitsShell("/opt/marengo", ["true"]);
    const at = (needle: string) => shell.indexOf(needle);
    const lock = shell.search(/flock -w \d+ /);
    const backup = at('_taught="$(mktemp -d)"');
    const merge = at("preserve-taught-limits.py");
    const rollback = at('cp -a "$_taught/$f" "$OPT/$f"');
    const unlock = shell.lastIndexOf("flock -u");
    assert.ok(lock >= 0 && lock < backup && backup < merge && merge < rollback && rollback < unlock, shell);
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
