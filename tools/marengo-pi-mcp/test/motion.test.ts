import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import type { MarengoPiConfig } from "../src/config.js";
import {
  benchCanKernelDeltaShell,
  benchCanKernelSnapshotShell,
  benchLogArchiveShell,
  expandScriptWithWaveWaits,
  holdSessionRemoteBody,
  marengoPiPipeLine,
  marengoPiPipeTimeoutSec,
  referenceAcquireLine,
  registerMotionTools,
  scriptSleepTotalSec,
} from "../src/tools/motion.js";
import { MASTER_JOINTS } from "../src/bench-profiles.js";
import { encodeRobotState } from "./robot-state-fixture.js";
import { gravityPreviewReply, isGravityPreviewBody } from "./gravity-fixture.js";

/**
 * Feed `lines` (through marengoPiPipeLine) into a fake marengo-pi that records stdin and
 * answers `home … sign-tested` with `replies` in $LOG, like marengo-pi stdout tee'd there.
 */
function runFeeder(lines: string[], replies: string[]) {
  const dir = mkdtempSync(path.join(tmpdir(), "ref-await-"));
  const bin = path.join(dir, "bin");
  mkdirSync(bin);
  writeFileSync(path.join(bin, "sleep"), "#!/bin/sh\nexit 0\n");
  writeFileSync(
    path.join(dir, "fake-pi"),
    [
      "#!/bin/bash",
      'while IFS= read -r l; do',
      `  printf '%s\\n' "$l" >> "${dir}/stdin"`,
      '  case "$l" in *sign-tested) cat "' + `${dir}/replies` + '" >> "$LOG" ;; esac',
      "done",
    ].join("\n"),
  );
  chmodSync(path.join(bin, "sleep"), 0o755);
  chmodSync(path.join(dir, "fake-pi"), 0o755);
  writeFileSync(path.join(dir, "replies"), replies.map((r) => `${r}\n`).join(""));
  writeFileSync(path.join(dir, "log"), "marengo-pi starting Disabled\n");
  const feeder = `{\n${lines.map(marengoPiPipeLine).join(";\n")};\n} | ${dir}/fake-pi`;
  const r = spawnSync("bash", ["-c", `set -euo pipefail\n${feeder}`], {
    env: { ...process.env, LOG: path.join(dir, "log"), PATH: `${bin}:${process.env.PATH ?? ""}` },
    encoding: "utf8",
  });
  return { status: r.status, stderr: r.stderr, stdin: readFileSync(path.join(dir, "stdin"), "utf8") };
}

describe("reference acquisition feeder", () => {
  const lines = [referenceAcquireLine(["j_a", "j_b"]), "home", "enable bench"];

  it("sends later lines only after every joint reports a current reference", () => {
    const r = runFeeder(lines, [
      "reference j_a current pos=0.0001",
      "reference j_b current pos=-0.0002",
    ]);
    assert.equal(r.status, 0, r.stderr);
    assert.equal(r.stdin, "home j_a j_b sign-tested\nhome\nenable bench\n");
  });

  it("fails fast on a failed joint: disable, quit, nonzero", () => {
    const r = runFeeder(lines, [
      "reference j_a current pos=0.0001",
      "reference j_b failed: mechPos readback outside tolerance",
    ]);
    assert.notEqual(r.status, 0);
    assert.equal(r.stdin, "home j_a j_b sign-tested\ndisable\nquit\n");
    assert.match(r.stderr, /reference acquisition failed \(j_a j_b\)/);
  });

  it("treats a parse-time refusal and a timeout as failures", () => {
    const refused = runFeeder(lines, ["home failed: unknown joint j_b"]);
    assert.equal(refused.stdin, "home j_a j_b sign-tested\ndisable\nquit\n");
    const silent = runFeeder(lines, ["reference j_a current pos=0.0000"]);
    assert.equal(silent.stdin, "home j_a j_b sign-tested\ndisable\nquit\n");
    assert.match(silent.stderr, /reference acquisition timeout/);
  });

  it("budgets the acquisition wait in the pipe timeout", () => {
    assert.equal(scriptSleepTotalSec([...lines, "sleep 2"]), 22);
  });

  it("recognizes --sign-tested anywhere on the line, as marengo-pi does", () => {
    assert.equal(scriptSleepTotalSec(["home --sign-tested j_a"]), 10);
    assert.match(marengoPiPipeLine("home --sign-tested j_a"), /grep -q '\^reference j_a current '/);
    assert.equal(marengoPiPipeLine("home"), `printf '%s\\n' "home"`);
    assert.equal(marengoPiPipeLine("home j_a"), `printf '%s\\n' "home j_a"`);
  });

  it("rejects joint names that are not identifiers", () => {
    assert.throws(() => referenceAcquireLine([]));
    assert.throws(() => referenceAcquireLine(["a;rm"]));
  });
});

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

describe("removed motor-repl tools", () => {
  it("does not advertise enable or jog without a working reference admission path", () => {
    const tools = registerMotionTools(cfg, async () => "", () => {});

    assert.equal("pi_motor_enable" in tools, false);
    assert.equal("pi_jog" in tools, false);
  });
});

describe("marengo-pi script tool", () => {
  it("uses timeout_sec as total pipe budget", () => {
    assert.equal(scriptSleepTotalSec(["hold-at 0", "sleep 2", "sleep 1.5"]), 3.5);
    assert.equal(marengoPiPipeTimeoutSec(["sleep 35", "disable"], 15), 15);
    assert.equal(marengoPiPipeTimeoutSec(["home", "disable"], 20), 20);
  });

  it("expands wave lines with auto wait sleep", () => {
    const expanded = expandScriptWithWaveWaits([
      "wave right_shoulder_roll 0.4 1.0 4",
      "hold-at right_shoulder_roll 0",
    ]);
    assert.deepEqual(expanded, [
      "wave right_shoulder_roll 0.4 1.0 4",
      "sleep 3.4",
      "hold-at right_shoulder_roll 0",
    ]);
  });

  it("keeps bench logs current when script exits nonzero", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_marengo_pi_script.handler({
      confirm: true,
      joint: "right_shoulder_pitch",
      script: ["home", "quit"],
      timeout_sec: 10,
    });

    assert.match(script, /\} 2>&1 \| tee -a "\$LOG"/);
    assert.match(script, /PIPE_STATUS=\$\{PIPESTATUS\[0\]\}/);
    assert.match(script, /ln -sf "\$LOG" "\$LOGDIR\/bench-latest\.log"/);
    assert.match(script, /position-trace-\$TS\.csv/);
    assert.match(script, /candump -t z/);
    assert.match(script, /candump-latest\.log/);
    assert.match(script, /trap restore_can_owner EXIT/);
    assert.match(script, /sudo -n '\/usr\/local\/libexec\/marengo\/pi-restart-marengo-pi\.sh' stop/);
    assert.doesNotMatch(script, /pkill -f/);
    assert.match(script, /can kernel start:/);
    assert.match(script, /can kernel delta/);
    assert.match(script, /exit "\$PIPE_STATUS"/);
  });

  it("sends disable and quit after a timed hold dwell", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        if (isGravityPreviewBody(body)) return gravityPreviewReply();
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_hold_on.handler({
      confirm: true,
      joint: "right_shoulder_pitch",
      set_zero: true,
      at_mechanical_reference: true,
      position_rad: 0,
      timeout_sec: 10,
    });

    assert.match(
      script,
      /printf '%s\\n' "home right_shoulder_pitch sign-tested"\n[\s\S]*grep -q '\^reference right_shoulder_pitch current '[\s\S]*fi;\n_ref_from=[^\n]*\nprintf '%s\\n' "home"\n[\s\S]*grep -q '\^homing verified '[\s\S]*fi;\n_ref_from=[^\n]*\nprintf '%s\\n' "enable bench"\n[\s\S]*grep -q '\^enabled \(operator='[\s\S]*fi;\nprintf '%s\\n' "hold-at right_shoulder_pitch 0";/,
    );
    assert.doesNotMatch(script, /motor-repl set-zero/);
    assert.match(script, /sleep 10;/);
    assert.match(script, /sleep 6;/);
    assert.match(script, /printf '%s\\n' "disable";/);
    assert.match(script, /printf '%s\\n' "quit";/);
    // 10 s dwell + 6 s return + 10 s acquisition budget (1 joint) + 10 s slack.
    assert.match(script, /\} \| timeout 36 \$PI_BIN/);
    assert.match(script, /bin\/motor-repl disable/);
  });

  it("refuses a hold without the reference opt-in before touching the Pi", async () => {
    let calls = 0;
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        calls += 1;
        return body;
      },
      () => {},
    );

    for (const optIn of [{}, { set_zero: true }, { at_mechanical_reference: true }]) {
      const out = await tools.pi_hold_on.handler({ confirm: true, position_rad: 0, ...optIn });
      assert.match(out, /^Refused: enabling needs a current reference/);
    }
    assert.equal(calls, 0);
  });

  it("references every profile joint when no hold joint is given", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        if (isGravityPreviewBody(body)) return gravityPreviewReply();
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_hold_on.handler({
      confirm: true,
      confirm_weighted_motion: true,
      profile: "roll_attached",
      set_zero: true,
      at_mechanical_reference: true,
    });

    assert.match(
      script,
      /"home right_shoulder_pitch right_shoulder_roll right_upper_arm_yaw sign-tested"/,
    );
    assert.match(script, /printf '%s\\n' "hold-on";/);
  });

  it("refuses a gravity model mismatch with zero motion calls", async () => {
    const bodies: string[] = [];
    const audits: number[] = [];
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        bodies.push(body);
        return isGravityPreviewBody(body)
          ? gravityPreviewReply({ right_shoulder_pitch: -1.5951, right_shoulder_roll: 0.02 })
          : "";
      },
      (_tool, _args, _out, exitCode) => audits.push(exitCode),
    );

    const out = await tools.pi_hold_on.handler({
      confirm: true,
      confirm_weighted_motion: true,
      profile: "roll_attached",
      set_zero: true,
      at_mechanical_reference: true,
    });

    assert.match(out, /^FAIL gravity_model_mismatch: \|τ_g\| at the hanging rest ≥ 0\.20 Nm on right_shoulder_pitch\./m);
    assert.match(out, /right_shoulder_pitch: q=0\.0000 rad τ_g=-1\.5951 Nm residual=1\.5951 Nm FAIL/);
    assert.match(out, /right_shoulder_roll: .* ok/);
    assert.deepEqual(audits, [1]);
    // Only the CAN-free snapshot read and the read-only preview (as sole CAN owner) ran.
    assert.equal(bodies.length, 2);
    assert.doesNotMatch(bodies[0], /motor-repl|marengo-pi\.sh' stop/);
    assert.match(bodies[1], /pi-restart-marengo-pi\.sh' stop[\s\S]*\nbin\/motor-repl gravity-preview$/);
    assert.ok(
      !bodies.some((b) => /sign-tested|enable|hold-on|hold-at|PI_BIN/.test(b)),
      "no motion command may reach the Pi",
    );
  });

  it("proceeds to the hold when the model matches the hanging arm", async () => {
    const bodies: string[] = [];
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        bodies.push(body);
        return isGravityPreviewBody(body)
          ? gravityPreviewReply({ right_shoulder_pitch: 0.012, right_shoulder_roll: -0.03 })
          : "hold session ran";
      },
      () => {},
    );

    const out = await tools.pi_hold_on.handler({
      confirm: true,
      confirm_weighted_motion: true,
      profile: "roll_attached",
      set_zero: true,
      at_mechanical_reference: true,
    });

    assert.match(out, /PASS gravity gate: \|τ_g\| at the hanging rest < 0\.20 Nm on every modeled joint\nhold session ran$/);
    assert.equal(bodies.length, 3);
    assert.match(bodies[2], /printf '%s\\n' "hold-on";/);
  });

  it("compares live drive torque with τ_g at the published pose while marengo-pi holds the arm", async () => {
    const nowMs = 1_790_000_000_000;
    const joints = [
      { name: "right_shoulder_pitch", homing: 3, driveActive: true, position: 0.48, effort: 0.57 },
      { name: "right_shoulder_roll", homing: 3, driveActive: true, position: 0, effort: 0.01 },
    ];
    const snapshot = Buffer.from(encodeRobotState(nowMs - 40, joints)).toString("base64");
    for (const [tauPitch, ok] of [[-0.1, false], [0.5, true]] as const) {
      const bodies: string[] = [];
      const tools = registerMotionTools(
        cfg,
        async (body) => {
          bodies.push(body);
          if (body.includes("snapshot/robot/state")) {
            return `pi_now_ms=${nowMs}\nrobot_state_b64=${snapshot}`;
          }
          return isGravityPreviewBody(body)
            ? gravityPreviewReply({ right_shoulder_pitch: tauPitch })
            : "hold session ran";
        },
        () => {},
      );
      const out = await tools.pi_hold_on.handler({
        confirm: true,
        joint: "right_shoulder_pitch",
        set_zero: true,
        at_mechanical_reference: true,
      });
      // The preview evaluates the published pose in robot.yaml order, not the zero pose.
      assert.match(bodies[1], /right_shoulder_pitch\) GG_Q="\$GG_Q 0\.48" ;;/);
      assert.match(out, /basis=measured/);
      if (ok) {
        assert.match(out, /PASS gravity gate: \|τ_meas − τ_g\| < 0\.20 Nm/);
        assert.equal(bodies.length, 3);
      } else {
        assert.match(out, /τ_meas=0\.5700 Nm residual=0\.6700 Nm FAIL/);
        assert.match(out, /FAIL gravity_model_mismatch: \|τ_meas − τ_g\| ≥ 0\.20 Nm on right_shoulder_pitch/);
        assert.equal(bodies.length, 2);
      }
    }
  });

  it("pi_motor_recover reads status while Disabled without reference or enable", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_motor_recover.handler({ confirm: true });

    assert.match(script, /\{ sleep 1; echo status; echo disable; echo quit; \} \| timeout 10 "\$PI_BIN"/);
    assert.doesNotMatch(script, /sign-tested|echo home|enable bench|set-zero/);
    assert.match(script, /RECOVER_SUMMARY/);
  });

  it("routes right_elbow_pitch holds to master config by default", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        if (isGravityPreviewBody(body)) return gravityPreviewReply();
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_hold_on.handler({
      confirm: true,
      joint: "right_elbow_pitch",
      set_zero: true,
      at_mechanical_reference: true,
      position_rad: 0.25,
      timeout_sec: 10,
    });

    assert.match(
      script,
      /export MARENGO_CONFIG_DIR='\/opt\/marengo\/config'/,
    );
  });

  it("uses master config for legacy bringup slug overrides", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        if (isGravityPreviewBody(body)) return gravityPreviewReply();
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_hold_on.handler({
      confirm: true,
      config_dir: "arm_3dof_right",
      joint: "right_shoulder_pitch",
      set_zero: true,
      at_mechanical_reference: true,
      position_rad: 0.1,
      timeout_sec: 10,
    });

    assert.match(
      script,
      /export MARENGO_CONFIG_DIR='\/opt\/marengo\/config'/,
    );
  });

  it("uses master config for homing status with legacy slug", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_homing_status.handler({
      config_dir: "arm_3dof_right",
    });

    assert.match(
      script,
      /export MARENGO_CONFIG_DIR='\/opt\/marengo\/config'/,
    );
  });

  it("treats sleep N script lines as shell dwell between marengo-pi commands", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_marengo_pi_script.handler({
      confirm: true,
      joint: "right_shoulder_pitch",
      script: ["home", "enable bench", "hold-at 1.570796", "sleep 35", "hold-at 0"],
      timeout_sec: 15,
    });

    assert.match(script, /printf '%s\\n' "hold-at 1\.570796";/);
    assert.match(script, /sleep 35;/);
    assert.match(script, /printf '%s\\n' "hold-at 0";/);
    assert.match(script, /printf '%s\\n' "quit";/);
    assert.match(script, /\} \| timeout 15 \$PI_BIN/);
  });

  it("pipes every script line into marengo-pi", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_marengo_pi_script.handler({
      confirm: true,
      joint: "right_shoulder_pitch",
      script: ["home", "enable bench", "status"],
      timeout_sec: 10,
    });

    assert.match(
      script,
      /\{\nprintf '%s\\n' "home";\nprintf '%s\\n' "enable bench";\nprintf '%s\\n' "status";\nprintf '%s\\n' "quit";\n\} \| timeout 10 \$PI_BIN/,
    );
  });

  it("selects the marengo-pi binary by path without executing it", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_marengo_pi_script.handler({
      confirm: true,
      joint: "right_shoulder_pitch",
      script: ["home", "disable"],
      timeout_sec: 10,
    });

    assert.match(script, /PI_BIN=bin\/marengo-pi/);
    assert.match(script, /PI_FALLBACK="\$HOME\/marengo\/target\/release\/marengo-pi"/);
    assert.match(script, /if ! test -x "\$PI_BIN" && test -x "\$PI_FALLBACK"; then\n  PI_BIN="\$PI_FALLBACK"\nfi/);
    // Running marengo-pi (even for `help`) opens SocketCAN before reading stdin.
    assert.doesNotMatch(script, /\$PI_BIN" 2>&1|PI_HELP/);
    assert.match(script, /\} \| timeout 10 \$PI_BIN/);
  });

  it("emits kernel counter snapshot helpers", () => {
    assert.match(benchCanKernelSnapshotShell("start"), /CAN_KERNEL_START/);
    assert.match(benchCanKernelSnapshotShell("end"), /CAN_KERNEL_END/);
    assert.match(benchCanKernelDeltaShell(), /can kernel delta/);
    assert.match(benchCanKernelDeltaShell(), /_rx0=\$\{_rx0:-0\}/);
    assert.match(benchCanKernelDeltaShell(), /grep -m1 -E "\^\[\[:space:\]\]\*\\\("/);
  });

  describe("benchCanKernelDeltaShell rates", () => {
    function delta(duration: string, start: string, end: string) {
      const dir = mkdtempSync(path.join(tmpdir(), "kernel-delta-"));
      writeFileSync(path.join(dir, "start"), start);
      writeFileSync(path.join(dir, "end"), end);
      writeFileSync(path.join(dir, "log"), "");
      const r = spawnSync("bash", ["-c", `set -euo pipefail\n${benchCanKernelDeltaShell()}`], {
        encoding: "utf8",
        env: {
          ...process.env,
          LOG: path.join(dir, "log"),
          CAN_KERNEL_START: path.join(dir, "start"),
          CAN_KERNEL_END: path.join(dir, "end"),
          CANDUMP_DURATION_SEC: duration,
        },
      });
      return { r, log: readFileSync(path.join(dir, "log"), "utf8") };
    }

    it("totals rx plus tx over the window", () => {
      const { r, log } = delta("10.000", "can0 1000 2000\n", "can0 1600 2400\n");
      assert.equal(r.status, 0, r.stderr);
      assert.equal(log, "can kernel delta can0: drx=600 dtx=400 total=100.0/s (10.000s window)\n");
    });

    it("reports counters without a rate for a zero window and does not fail the session", () => {
      for (const window of ["0.000", "0"]) {
        const { r, log } = delta(window, "can0 1000 2000\n", "can0 1600 2400\n");
        assert.equal(r.status, 0, r.stderr);
        assert.equal(r.stderr, "");
        assert.equal(log, "can kernel delta can0: drx=600 dtx=400\n");
      }
    });
  });

  it("uses the installed absolute log CLI and preserves files when unavailable", () => {
    const root = mkdtempSync(path.join(tmpdir(), "archive-cli-missing-"));
    const log = path.join(root, "bench-latest.log");
    writeFileSync(log, "unarchived evidence");

    const shell = benchLogArchiveShell(root, 2);
    const r = spawnSync(
      "bash",
      [
        "-c",
        `set -euo pipefail\nTS=20261003T120000Z LABEL=test LOG=${log} TRACE=${log}.csv CANDUMP= LOGDIR=${root}\n${shell}`,
      ],
      { encoding: "utf8" },
    );

    assert.equal(r.status, 0, r.stderr);
    assert.ok(shell.includes(`${root}/bin/marengo-log-cli' session register`));
    assert.doesNotMatch(shell, /command -v marengo-log-cli|rm -f/);
    assert.equal(readFileSync(log, "utf8"), "unarchived evidence");
  });

  it("does not archive hot files after session registration fails", () => {
    const root = mkdtempSync(path.join(tmpdir(), "archive-register-fails-"));
    const bin = path.join(root, "bin");
    mkdirSync(bin);
    const log = path.join(root, "bench-latest.log");
    const calls = path.join(root, "calls");
    writeFileSync(log, "unarchived evidence");
    writeFileSync(
      path.join(bin, "marengo-log-cli"),
      [
        "#!/bin/bash",
        'printf "%s %s\\n" "$1" "$2" >> "$CALLS"',
        'if [[ "$2" == register ]]; then exit 1; fi',
        'if [[ "$2" == archive ]]; then rm -f "$LOG"; fi',
      ].join("\n"),
    );
    chmodSync(path.join(bin, "marengo-log-cli"), 0o755);

    const shell = benchLogArchiveShell(root, 2);
    const r = spawnSync(
      "bash",
      [
        "-c",
        `set -euo pipefail\nTS=20261003T120000Z LABEL=test LOG=${log} TRACE=${log}.csv CANDUMP= CALLS=${calls}\nexport LOG CALLS\n${shell}`,
      ],
      { encoding: "utf8" },
    );

    assert.equal(r.status, 0, r.stderr);
    assert.equal(readFileSync(calls, "utf8"), "session register\n");
    assert.equal(readFileSync(log, "utf8"), "unarchived evidence");
  });

  it("omits --candump when the session has no capture", () => {
    const shell = benchLogArchiveShell("/opt/marengo");

    assert.match(shell, /if \[\[ -n "\$\{CANDUMP:-\}" \]\]; then CANDUMP_ARGS=\(--candump "\$CANDUMP"\)/);
    assert.match(shell, /"\$\{CANDUMP_ARGS\[@\]\}"/);
    assert.doesNotMatch(shell, /--candump "\$\{CANDUMP:-\}"/);
  });
});

/**
 * Run the hold session body against fake bin/motor-repl and bin/marengo-pi that append to
 * $TRACE, with instant `sleep`, pass-through `timeout`, silent `ip` and sysfs CAN counters
 * under MARENGO_CAN_SYSFS. `lingerPgrep` makes pgrep report marengo-pi for that many calls
 * after marengo-pi exits; `bumpOnSleep` raises rx_over_errors on every sleep (overrunning bus).
 * The fake marengo-pi records its stdin and answers `home` / `enable …` like marengo-pi
 * (`enableReply` replaces the accepted-enable line).
 */
function runHoldBody(
  opts: { lingerPgrep?: number; bumpOnSleep?: boolean; enableReply?: string } = {},
) {
  const dir = mkdtempSync(path.join(tmpdir(), "hold-body-"));
  const fake = path.join(dir, "fakebin");
  const stats = path.join(dir, "sys", "can0", "statistics");
  mkdirSync(path.join(dir, "bin"));
  mkdirSync(fake);
  mkdirSync(stats, { recursive: true });
  for (const counter of ["rx_errors", "rx_over_errors", "tx_errors"]) {
    writeFileSync(path.join(stats, counter), "0\n");
  }
  const owners = path.join(dir, "owners");
  const scripts: Record<string, string> = {
    "bin/motor-repl": 'echo "motor-repl $*" >> "$TRACE"',
    "bin/marengo-pi": [
      'echo "marengo-pi start" >> "$TRACE"',
      'while IFS= read -r l; do',
      `  printf '%s\\n' "$l" >> ${path.join(dir, "stdin")}`,
      '  case "$l" in',
      '    *sign-tested) for j in ${l#home }; do [[ $j == sign-tested ]] || echo "reference $j current pos=0.0000" >> "$LOG"; done ;;',
      '    home) echo "homing verified → Ready" >> "$LOG" ;;',
      `    enable*) echo ${JSON.stringify(opts.enableReply ?? "enabled (operator=bench) targets=right_shoulder_pitch")} >> "$LOG" ;;`,
      "    quit) break ;;",
      "  esac",
      "done",
      'echo "marengo-pi exit" >> "$TRACE"',
      `echo ${opts.lingerPgrep ?? 0} > ${owners}`,
    ].join("\n"),
    "fakebin/sleep": opts.bumpOnSleep
      ? `f=${path.join(stats, "rx_over_errors")}; echo $(( $(cat "$f") + 1 )) > "$f"`
      : "exit 0",
    "fakebin/timeout": 'shift; exec "$@"',
    "fakebin/ip": "exit 0",
    "fakebin/pgrep": [
      `n=$(cat ${owners} 2>/dev/null || echo 0)`,
      "if (( n > 0 )); then",
      `  echo $((n - 1)) > ${owners}`,
      '  echo "4242 marengo-pi"',
      "  exit 0",
      "fi",
      "exit 1",
    ].join("\n"),
  };
  for (const [rel, body] of Object.entries(scripts)) {
    writeFileSync(path.join(dir, rel), `#!/bin/bash\n${body}\n`);
    chmodSync(path.join(dir, rel), 0o755);
  }
  const trace = path.join(dir, "trace");
  const log = path.join(dir, "log");
  writeFileSync(trace, "");
  writeFileSync(log, "");
  writeFileSync(path.join(dir, "stdin"), "");
  const body = holdSessionRemoteBody(cfg, {
    joint: "right_shoulder_pitch",
    referenceJoints: [...MASTER_JOINTS],
    operator: "bench",
    timeoutSec: 1,
    returnHomeSec: 1,
  });
  const r = spawnSync("bash", ["-c", `set -uo pipefail\n${body}`], {
    cwd: dir,
    env: {
      ...process.env,
      LOG: log,
      TRACE: trace,
      MARENGO_CAN_SYSFS: path.join(dir, "sys"),
      PATH: `${fake}:${process.env.PATH ?? ""}`,
    },
    encoding: "utf8",
  });
  return {
    status: r.status,
    stdout: r.stdout,
    stderr: r.stderr,
    trace: readFileSync(trace, "utf8").trim().split("\n").filter(Boolean),
    stdin: readFileSync(path.join(dir, "stdin"), "utf8"),
  };
}

describe("hold session CAN ownership (Transport race)", () => {
  it("arm_attached gates and references all five right-arm joints", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        if (isGravityPreviewBody(body)) return gravityPreviewReply();
        script = body;
        return "hold session ran";
      },
      () => {},
    );

    const out = await tools.pi_hold_on.handler({
      confirm: true,
      confirm_weighted_motion: true,
      profile: "arm_attached",
      set_zero: true,
      at_mechanical_reference: true,
    });

    for (const joint of MASTER_JOINTS) {
      assert.match(out, new RegExp(`  ${joint}: q=0\\.0000 rad τ_g=0\\.0000 Nm residual=0\\.0000 Nm ok`));
    }
    assert.doesNotMatch(out, /left_shoulder_pitch|not in the gravity model/);
    assert.match(script, new RegExp(`"home ${MASTER_JOINTS.join(" ")} sign-tested"`));
  });

  it("runs exactly one motor-repl before marengo-pi, then settles CAN before launch", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        if (isGravityPreviewBody(body)) return gravityPreviewReply();
        script = body;
        return body;
      },
      () => {},
    );
    await tools.pi_hold_on.handler({
      confirm: true,
      set_zero: true,
      at_mechanical_reference: true,
    });

    const launch = script.indexOf("} | timeout ");
    assert.ok(launch > 0);
    const beforeLaunch = script.slice(0, launch);
    assert.equal(beforeLaunch.match(/bin\/motor-repl /g)?.length, 1);
    // lastIndexOf: the EXIT-trap restore also defines a settle, ahead of the session body.
    assert.ok(
      beforeLaunch.indexOf("bin/motor-repl disable") < beforeLaunch.lastIndexOf("can_error_counters() {"),
      "the pre-session disable precedes the CAN settle",
    );
    assert.match(script.slice(launch), /can errors after marengo-pi[\s\S]*bin\/motor-repl disable/);
  });

  it("starts marengo-pi on a settled bus and disables only after marengo-pi exits", () => {
    const r = runHoldBody();
    assert.equal(r.status, 0, r.stderr);
    assert.equal(r.trace[0], "marengo-pi start");
    assert.ok(!r.trace.slice(0, r.trace.indexOf("marengo-pi exit")).some((l) => l.startsWith("motor-repl")));
    assert.deepEqual(r.trace.slice(-2), ["marengo-pi exit", "motor-repl disable"]);
    assert.match(r.stdout, /^can settle: ok \(0\.5s quiet, no CAN owner\) can0 rx_errors=0 rx_over=0 tx_errors=0 /m);
    assert.match(r.stdout, /^can errors after marengo-pi: can0 rx_errors=0 rx_over=0 /m);
  });

  it("waits for the readiness check and enable before sending the hold", () => {
    const r = runHoldBody();
    assert.equal(r.status, 0, r.stderr);
    assert.equal(
      r.stdin,
      [
        `home ${MASTER_JOINTS.join(" ")} sign-tested`,
        "home",
        "enable bench",
        "hold-on",
        "hold-at right_shoulder_pitch 0",
        "status",
        "disable",
        "quit",
        "",
      ].join("\n"),
    );
  });

  it("ends the session on a refused enable: disable and quit, never the hold", () => {
    // 2026-10-03 15:34 bench: after `enable failed:` the old feeder still sent hold-at.
    for (const reply of [
      "enable failed: homing verify on right_shoulder_pitch: device identity reply missing at admission",
      "enable blocked: persistent safety fault 1 (DriveState)",
    ]) {
      const r = runHoldBody({ enableReply: reply });
      assert.notEqual(r.status, 0);
      assert.equal(r.stdin, `home ${MASTER_JOINTS.join(" ")} sign-tested\nhome\nenable bench\ndisable\nquit\n`);
      assert.match(r.stderr, /enable failed \(enable bench\); sending disable\/quit/);
      assert.deepEqual(r.trace.slice(-2), ["marengo-pi exit", "motor-repl disable"]);
    }
  });

  it("ends the session on an unanswered enable", () => {
    const r = runHoldBody({ enableReply: "still thinking" });
    assert.notEqual(r.status, 0);
    assert.equal(r.stdin, `home ${MASTER_JOINTS.join(" ")} sign-tested\nhome\nenable bench\ndisable\nquit\n`);
    assert.match(r.stderr, /enable timeout \(enable bench\); sending disable\/quit/);
  });

  it("pi_hold_on names the refused enable and audits a failure", async () => {
    const audits: number[] = [];
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        if (isGravityPreviewBody(body)) return gravityPreviewReply();
        if (!body.includes("LABEL='hold-on'")) return "";
        return [
          "homing verified → Ready",
          "enable failed: homing verify on right_shoulder_pitch: device identity reply missing at admission",
          "enable failed (enable bench); sending disable/quit",
          "[exit 1]",
        ].join("\n");
      },
      (_tool, _args, _out, exitCode) => audits.push(exitCode),
    );
    const out = await tools.pi_hold_on.handler({ confirm: true, set_zero: true, at_mechanical_reference: true });
    assert.match(
      out,
      /marengo-pi refused the session before the hold: enable failed: homing verify on right_shoulder_pitch: device identity reply missing at admission\n/,
    );
    assert.deepEqual(audits, [1]);
  });

  it("refuses to start marengo-pi while CAN error counters keep moving", () => {
    const r = runHoldBody({ bumpOnSleep: true });
    assert.equal(r.status, 1);
    assert.deepEqual(r.trace, []);
    assert.match(r.stderr, /can settle: FAIL/);
    assert.match(r.stderr, /refusing to start marengo-pi/);
  });

  it("skips the post-session motor-repl disable while marengo-pi still owns CAN", () => {
    const r = runHoldBody({ lingerPgrep: 40 });
    assert.equal(r.status, 1);
    assert.equal(r.trace[r.trace.length - 1], "marengo-pi exit");
    assert.ok(!r.trace.some((l) => l.startsWith("motor-repl")));
    assert.match(r.stdout, /post-session bin\/motor-repl disable skipped: marengo-pi \(pid 4242\) owns CAN/);
  });
});
