import type { BenchProfile, MarengoPiConfig } from "../config.js";
import { sudoCanUpCommand } from "../config.js";
import {
  homingPreflightShell,
  homingStatusOutputOk,
} from "../homing-preflight.js";
import { shellQuote, wrapRemoteWithConfig } from "../env.js";
import { benchLogArchiveShell, benchCandumpStartShell, benchCandumpStopShell, scriptSleepTotalSec } from "../tools/motion.js";

const BENCH_CONFIG_WEIGHTED =
  "/opt/marengo/config/bringup/shoulder_pitch_weighted";
const BENCH_CONFIG_3DOF = "/opt/marengo/config/bringup/arm_3dof_right";

const ROLL_JOINT = "right_shoulder_roll";
const PITCH_JOINT = "right_shoulder_pitch";
const YAW_JOINT = "right_upper_arm_yaw";

function holdAt(joint: string, rad: number): string {
  return `hold-at ${joint} ${rad}`;
}

function rollReturnHomeLines(): string[] {
  return [
    holdAt(ROLL_JOINT, 0.05),
    "sleep 15",
    holdAt(ROLL_JOINT, 0.02),
    "sleep 15",
    holdAt(ROLL_JOINT, 0),
    "sleep 25",
  ];
}

/** Staged descent only — use when roll is elevated (q > ~0.2 rad). Do not run from home. */
export function rollStagedDescentLines(): string[] {
  return [
    holdAt(ROLL_JOINT, 1.0),
    "sleep 15",
    holdAt(ROLL_JOINT, 0.7),
    "sleep 15",
    holdAt(ROLL_JOINT, 0.4),
    "sleep 15",
    holdAt(ROLL_JOINT, 0.15),
    "sleep 12",
    holdAt(ROLL_JOINT, 0.05),
    "sleep 12",
    holdAt(ROLL_JOINT, 0),
    "sleep 15",
  ];
}

function isRightArmBenchProfile(profile: BenchProfile): boolean {
  return profile === "roll_attached" || profile === "arm_2dof_smoke";
}

function harnessSetZeroJoints(profile: BenchProfile): string[] {
  if (isRightArmBenchProfile(profile)) {
    return [ROLL_JOINT, PITCH_JOINT, YAW_JOINT];
  }
  return ["left_shoulder_pitch", "right_shoulder_pitch"];
}

/** Config profile for harness runs; weighted profile must use shoulder_pitch_weighted URDF. */
export function harnessConfigDir(
  cfg: MarengoPiConfig,
  profile: BenchProfile,
  configDir?: string,
): string {
  if (configDir) {
    if (configDir.startsWith("/") || configDir.startsWith("~/")) {
      return configDir;
    }
    return `${cfg.piRoot}/config/bringup/${configDir}`;
  }
  if (isRightArmBenchProfile(profile)) {
    return BENCH_CONFIG_3DOF;
  }
  if (profile === "weighted_single_arm" || profile === "arm_attached") {
    return BENCH_CONFIG_WEIGHTED;
  }
  return cfg.configDir;
}

export interface HarnessStep {
  name: string;
  ok: boolean;
  output: string;
}

export interface HarnessResult {
  profile: BenchProfile;
  pass: boolean;
  loaded_joint?: string;
  steps: HarnessStep[];
  faults: string[];
  log_path?: string;
}

export interface HarnessArgs {
  profile?: BenchProfile;
  config_dir?: string;
  joints?: string[];
  loaded_joint?: string;
  gravity_angles?: number[];
  skip_set_zero?: boolean;
  debug?: boolean;
}

function marengoPiPipeLine(line: string): string {
  const sleepMatch = /^sleep (\d+(?:\.\d+)?)$/i.exec(line.trim());
  if (sleepMatch) {
    return `sleep ${sleepMatch[1]}`;
  }
  return `printf '%s\\n' ${JSON.stringify(line)}`;
}

function marengoPiPipe(script: string[], timeoutSec: number): string {
  const pipeLines = script.map(marengoPiPipeLine).join(";\n");
  return `{\n${pipeLines};\n} | timeout ${timeoutSec} bin/marengo-pi`;
}

/** Sleep sum + motion/startup slack for marengo-pi pipe timeout. */
function pipeTimeoutSec(script: string[], minSec = 30): number {
  return Math.max(minSec, Math.ceil(scriptSleepTotalSec(script) + 15));
}

function benchSessionWrapper(
  cfg: MarengoPiConfig,
  configDir: string,
  label: string,
  pipeCmd: string,
  debug: boolean,
): string {
  const logDir = `${cfg.piRoot}/var/log`;
  return wrapRemoteWithConfig(
    cfg,
    [
      `LOGDIR=${shellQuote(logDir)}`,
      "mkdir -p \"$LOGDIR\"",
      'TS=$(date -u +"%Y%m%dT%H%M%SZ")',
      `LOG="$LOGDIR/bench-$TS.log"`,
      `TRACE="$LOGDIR/position-trace-$TS.csv"`,
      `JSON="$LOGDIR/bench-$TS.json"`,
      'export MARENGO_POSITION_TRACE="$TRACE"',
      'export MARENGO_POSITION_TRACE_HZ="${MARENGO_POSITION_TRACE_HZ:-50}"',
      'export MARENGO_LOG_SESSION_ID="$TS"',
      `LABEL=${shellQuote(label)}`,
      "echo \"=== bench harness $TS ($LABEL) ===\" | tee \"$LOG\"",
      benchCandumpStartShell(),
      "set +e",
      "{",
      pipeCmd,
      "} 2>&1 | tee -a \"$LOG\"",
      "PIPE_STATUS=${PIPESTATUS[0]}",
      "set -e",
      benchCandumpStopShell(),
      'ln -sf "$LOG" "$LOGDIR/bench-latest.log"',
      'ln -sf "$TRACE" "$LOGDIR/position-trace-latest.csv"',
      benchLogArchiveShell(cfg.piRoot),
      "echo \"log=$LOG candump=${CANDUMP:-}\"",
      "exit \"$PIPE_STATUS\"",
    ].join("\n"),
    configDir,
    debug,
  );
}

export async function runBenchHarness(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  args: HarnessArgs,
): Promise<string> {
  const profile = args.profile ?? cfg.benchProfile;
  const configDir = harnessConfigDir(cfg, profile, args.config_dir);
  const loadedJoint = args.loaded_joint ?? cfg.loadedJoint;
  const debug = args.debug ?? false;
  const steps: HarnessStep[] = [];
  const faults: string[] = [];
  let logPath: string | undefined;

  const remote = (body: string) => wrapRemoteWithConfig(cfg, body, configDir, debug);

  async function step(
    name: string,
    body: string,
    timeoutMs: number,
    isOk: (out: string) => boolean = defaultStepOk,
  ): Promise<boolean> {
    const out = await runRemote(body, timeoutMs);
    const ok = isOk(out);
    steps.push({ name, ok, output: out.slice(0, 4000) });
    if (!ok) {
      const faultLines = out
        .split("\n")
        .filter((l) =>
          /\berror\b|\bwarn\b|fault=0x[0-9a-fA-F]*[1-9a-fA-F]|watchdog|outside \[/i.test(
            l,
          ),
        );
      faults.push(...faultLines.slice(0, 20));
    }
    const logMatch = out.match(/log=(\S+)/);
    if (logMatch) logPath = logMatch[1];
    return ok;
  }

  function defaultStepOk(out: string): boolean {
    const exitMatch = out.match(/\[exit (\d+)\]/);
    if (exitMatch) {
      const code = Number(exitMatch[1]);
      // marengo-pi often exits 2 after disable/quit despite a clean session
      if (code !== 0 && code !== 2) {
        return false;
      }
    }
    if (out.toLowerCase().includes("control tick failed")) {
      return false;
    }
    if (/fault=0x[0-9a-fA-F]*[1-9a-fA-F]/.test(out)) {
      return false;
    }
    if (/watchdog|outside \[/i.test(out)) {
      return false;
    }
    return true;
  }

  // 1. health + can up
  const healthBody = remote(
    [
      "ip -br link show type can || true",
      "test -x bin/marengo-pi",
      "cat .deploy-rev 2>/dev/null || true",
    ].join("\n"),
  );
  if (!(await step("health", healthBody, 30_000))) {
    return formatHarnessResult(profile, loadedJoint, steps, faults, logPath);
  }

  const canUpBody = remote(sudoCanUpCommand(cfg));
  if (!(await step("can_up", canUpBody, 60_000))) {
    return formatHarnessResult(profile, loadedJoint, steps, faults, logPath);
  }

  // 2. motor-repl status
  if (!(await step("motor_repl_status", remote("bin/motor-repl status"), 30_000))) {
    return formatHarnessResult(profile, loadedJoint, steps, faults, logPath);
  }

  // 3. set-zero (skipped by default)
  if (!args.skip_set_zero) {
    for (const joint of harnessSetZeroJoints(profile)) {
      const body = remote(`bin/motor-repl set-zero ${joint}`);
      if (!(await step(`set_zero_${joint}`, body, 30_000))) {
        return formatHarnessResult(profile, loadedJoint, steps, faults, logPath);
      }
    }
  } else {
    steps.push({
      name: "set_zero_skipped",
      ok: true,
      output: "skip_set_zero=true (default)",
    });
  }

  // 3b. homing preflight — fail before motion if calibration record / Verified missing
  if (
    !(await step(
      "homing_preflight",
      remote(homingPreflightShell(true)),
      30_000,
      homingStatusOutputOk,
    ))
  ) {
    return formatHarnessResult(profile, loadedJoint, steps, faults, logPath);
  }

  // 4. gravity-preview 0 0 (single-joint / dual-pitch profiles only)
  if (!isRightArmBenchProfile(profile)) {
    if (
      !(await step(
        "gravity_preview_0_0",
        remote("bin/motor-repl gravity-preview 0 0"),
        30_000,
      ))
    ) {
      return formatHarnessResult(profile, loadedJoint, steps, faults, logPath);
    }
  } else {
    steps.push({
      name: "gravity_preview_skipped",
      ok: true,
      output: "right-arm bench profile — no gravity preview step",
    });
  }

  if (profile === "roll_attached") {
    const pitchHold0 = holdAt(PITCH_JOINT, 0);
    const scripts: { name: string; lines: string[]; timeoutSec: number }[] = [
      {
        name: "roll_sign_probe",
        timeoutSec: 40,
        lines: [
          "home",
          "enable bench",
          pitchHold0,
          "sleep 2",
          holdAt(ROLL_JOINT, 0.15),
          "sleep 8",
          ...rollReturnHomeLines(),
          "status",
          "disable",
          "quit",
        ],
      },
      {
        name: "roll_hold_sweep",
        timeoutSec: 90,
        lines: [
          "home",
          "enable bench",
          pitchHold0,
          "sleep 2",
          holdAt(ROLL_JOINT, 0.15),
          "sleep 12",
          holdAt(ROLL_JOINT, 0.785),
          "sleep 15",
          holdAt(ROLL_JOINT, 1.2),
          "sleep 15",
          holdAt(ROLL_JOINT, 1.57),
          "sleep 15",
          ...rollReturnHomeLines(),
          "status",
          "disable",
          "quit",
        ],
      },
      {
        name: "roll_round_trip",
        timeoutSec: 70,
        lines: [
          "home",
          "enable bench",
          pitchHold0,
          "sleep 2",
          holdAt(ROLL_JOINT, 1.57),
          "sleep 25",
          ...rollReturnHomeLines(),
          "status",
          "disable",
          "quit",
        ],
      },
    ];

    for (const s of scripts) {
      const pipeSec = pipeTimeoutSec(s.lines, s.timeoutSec);
      const pipeCmd = marengoPiPipe(s.lines, pipeSec);
      const body = benchSessionWrapper(cfg, configDir, s.name, pipeCmd, debug);
      if (!(await step(s.name, body, (pipeSec + 30) * 1000))) {
        break;
      }
    }
  } else if (profile === "arm_2dof_smoke") {
    const scripts: { name: string; lines: string[]; timeoutSec: number }[] = [
      {
        name: "smoke_pitch_hold",
        timeoutSec: 40,
        lines: [
          "home",
          "enable bench",
          holdAt(ROLL_JOINT, 0),
          "sleep 2",
          holdAt(PITCH_JOINT, 0.3),
          "sleep 10",
          holdAt(PITCH_JOINT, 0),
          "sleep 10",
          "status",
          "disable",
          "quit",
        ],
      },
      {
        name: "smoke_roll_hold",
        timeoutSec: 45,
        lines: [
          "home",
          "enable bench",
          holdAt(PITCH_JOINT, 0),
          "sleep 2",
          holdAt(ROLL_JOINT, 0.785),
          "sleep 15",
          holdAt(ROLL_JOINT, 0),
          "sleep 15",
          "status",
          "disable",
          "quit",
        ],
      },
      {
        name: "smoke_cross_talk",
        timeoutSec: 50,
        lines: [
          "home",
          "enable bench",
          holdAt(ROLL_JOINT, 0),
          "sleep 2",
          holdAt(PITCH_JOINT, 0.3),
          "sleep 8",
          "status",
          holdAt(PITCH_JOINT, 0),
          "sleep 8",
          holdAt(ROLL_JOINT, 0.785),
          "sleep 8",
          "status",
          holdAt(ROLL_JOINT, 0),
          "sleep 8",
          "disable",
          "quit",
        ],
      },
    ];

    for (const s of scripts) {
      const pipeSec = pipeTimeoutSec(s.lines, s.timeoutSec);
      const pipeCmd = marengoPiPipe(s.lines, pipeSec);
      const body = benchSessionWrapper(cfg, configDir, s.name, pipeCmd, debug);
      if (!(await step(s.name, body, (pipeSec + 30) * 1000))) {
        break;
      }
    }
  } else if (profile === "bare_motor") {
    const scripts: { name: string; lines: string[] }[] = [
      {
        name: "right_only_gravity",
        lines: ["home", "enable bench", "status", "gravity-on", "status", "disable", "quit"],
      },
      {
        name: "left_only_gravity",
        lines: ["home", "enable bench", "status", "gravity-on", "status", "disable", "quit"],
      },
      {
        name: "both_gravity",
        lines: ["home", "enable bench", "status", "gravity-on", "status", "disable", "quit"],
      },
    ];

    for (const s of scripts) {
      const pipeCmd = marengoPiPipe(s.lines, 35);
      const body = benchSessionWrapper(cfg, configDir, s.name, pipeCmd, debug);
      if (!(await step(s.name, body, 45_000))) {
        break;
      }
    }
  } else if (profile === "weighted_single_arm") {
    const angles = args.gravity_angles ?? [0, 0.3, -0.3];
    for (const a of angles) {
      const q0 = loadedJoint === "left_shoulder_pitch" ? a : 0;
      const q1 = loadedJoint === "right_shoulder_pitch" ? a : 0;
      const body = remote(`bin/motor-repl gravity-preview ${q0} ${q1}`);
      if (!(await step(`gravity_preview_${a}`, body, 30_000))) {
        return formatHarnessResult(profile, loadedJoint, steps, faults, logPath);
      }
    }

    const pipeCmd = marengoPiPipe(
      ["home", "enable bench", "status", "gravity-on", "status", "disable", "quit"],
      40,
    );
    const body = benchSessionWrapper(cfg, configDir, "weighted_gravity_on", pipeCmd, debug);
    await step("weighted_gravity_on", body, 50_000);
  }

  // final disable + status
  await step("final_disable", remote("bin/motor-repl disable"), 15_000);
  await step("final_status", remote("bin/motor-repl status"), 15_000);

  return formatHarnessResult(profile, loadedJoint, steps, faults, logPath);
}

function formatHarnessResult(
  profile: BenchProfile,
  loadedJoint: string | undefined,
  steps: HarnessStep[],
  faults: string[],
  logPath: string | undefined,
): string {
  const pass = steps.every((s) => s.ok) && faults.length === 0;
  const result: HarnessResult = {
    profile,
    pass,
    loaded_joint: loadedJoint,
    steps: steps.map((s) => ({ name: s.name, ok: s.ok, output: s.output.slice(0, 500) })),
    faults,
    log_path: logPath,
  };

  const summary = JSON.stringify(result, null, 2);
  const human = steps
    .map((s) => `[${s.ok ? "PASS" : "FAIL"}] ${s.name}`)
    .join("\n");

  return `${human}\n\n--- JSON ---\n${summary}`;
}
