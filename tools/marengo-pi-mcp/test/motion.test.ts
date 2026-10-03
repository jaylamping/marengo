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
  benchLogPruneShell,
  expandScriptWithWaveWaits,
  marengoPiPipeLine,
  marengoPiPipeTimeoutSec,
  referenceAcquireLine,
  registerMotionTools,
  scriptSleepTotalSec,
} from "../src/tools/motion.js";

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
      /printf '%s\\n' "home right_shoulder_pitch sign-tested"\n[\s\S]*grep -q '\^reference right_shoulder_pitch current '[\s\S]*fi;\nprintf '%s\\n' "home";\nprintf '%s\\n' "enable bench";\nprintf '%s\\n' "hold-at right_shoulder_pitch 0";/,
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

  it("reads homing from the gateway instead of motor-repl while CAN is owned", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return "robot_state_b64=";
      },
      () => {},
    );

    const out = await tools.pi_homing_status.handler({});

    const ownedStart = script.indexOf('if [[ -n "$CAN_OWNER" ]]; then');
    const elseAt = script.indexOf("\nelse\n", ownedStart);
    assert.ok(ownedStart >= 0 && elseAt > ownedStart);
    assert.doesNotMatch(script.slice(ownedStart, elseAt), /^bin\/motor-repl/m);
    assert.match(script.slice(ownedStart, elseAt), /snapshot\/robot\/state/);
    assert.match(script.slice(elseAt), /^bin\/motor-repl homing-status$/m);
    assert.match(out, /RobotState: unavailable/);
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
});
