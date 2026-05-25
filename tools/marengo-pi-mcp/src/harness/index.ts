import type { BenchProfile, MarengoPiConfig } from "../config.js";
import { sudoCanUpCommand } from "../config.js";
import { shellQuote, wrapRemote } from "../env.js";

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
  joints?: string[];
  loaded_joint?: string;
  gravity_angles?: number[];
  skip_set_zero?: boolean;
  debug?: boolean;
}

function marengoScript(
  lines: string[],
  timeoutSec: number,
): string {
  const printfLines = lines
    .map((l) => `printf '%s\\n' ${JSON.stringify(l)}`)
    .join("\n");
  return `${printfLines} | timeout ${timeoutSec} bin/marengo-pi`;
}

function benchSessionWrapper(
  cfg: MarengoPiConfig,
  label: string,
  inner: string,
  debug: boolean,
): string {
  const logDir = `${cfg.piRoot}/var/log`;
  return wrapRemote(
    cfg,
    [
      `LOGDIR=${shellQuote(logDir)}`,
      "mkdir -p \"$LOGDIR\"",
      'TS=$(date -u +"%Y%m%dT%H%M%SZ")',
      `LOG="$LOGDIR/bench-$TS.log"`,
      `JSON="$LOGDIR/bench-$TS.json"`,
      `LABEL=${shellQuote(label)}`,
      "echo \"=== bench harness $TS ($LABEL) ===\" | tee \"$LOG\"",
      `{ ${inner}; } 2>&1 | tee -a "$LOG"`,
      "ln -sf \"$LOG\" \"$LOGDIR/bench-latest.log\"",
      "echo \"log=$LOG\"",
    ].join("\n"),
    debug,
  );
}

export async function runBenchHarness(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  args: HarnessArgs,
): Promise<string> {
  const profile = args.profile ?? cfg.benchProfile;
  const loadedJoint = args.loaded_joint ?? cfg.loadedJoint;
  const debug = args.debug ?? false;
  const steps: HarnessStep[] = [];
  const faults: string[] = [];
  let logPath: string | undefined;

  async function step(name: string, body: string, timeoutMs: number): Promise<boolean> {
    const out = await runRemote(body, timeoutMs);
    const ok =
      !out.includes("[exit ") &&
      !out.toLowerCase().includes("failed") &&
      !out.includes("fault");
    steps.push({ name, ok, output: out.slice(0, 4000) });
    if (!ok) {
      const faultLines = out
        .split("\n")
        .filter((l) => /error|warn|fault|watchdog|outside|limit/i.test(l));
      faults.push(...faultLines.slice(0, 20));
    }
    const logMatch = out.match(/log=(\S+)/);
    if (logMatch) logPath = logMatch[1];
    return ok;
  }

  // 1. health + can up
  const healthBody = wrapRemote(
    cfg,
    [
      "ip -br link show type can || true",
      "test -x bin/marengo-pi",
      "cat .deploy-rev 2>/dev/null || true",
    ].join("\n"),
    debug,
  );
  if (!(await step("health", healthBody, 30_000))) {
    return formatHarnessResult(profile, loadedJoint, steps, faults, logPath);
  }

  const canUpBody = wrapRemote(cfg, sudoCanUpCommand(cfg), debug);
  if (!(await step("can_up", canUpBody, 60_000))) {
    return formatHarnessResult(profile, loadedJoint, steps, faults, logPath);
  }

  // 2. motor-repl status
  if (!(await step("motor_repl_status", wrapRemote(cfg, "bin/motor-repl status", debug), 30_000))) {
    return formatHarnessResult(profile, loadedJoint, steps, faults, logPath);
  }

  // 3. set-zero (skipped by default)
  if (!args.skip_set_zero) {
    for (const joint of ["left_shoulder_pitch", "right_shoulder_pitch"]) {
      const body = wrapRemote(cfg, `bin/motor-repl set-zero ${joint}`, debug);
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

  // 4. gravity-preview 0 0
  if (!(await step("gravity_preview_0_0", wrapRemote(cfg, "bin/motor-repl gravity-preview 0 0", debug), 30_000))) {
    return formatHarnessResult(profile, loadedJoint, steps, faults, logPath);
  }

  if (profile === "bare_motor") {
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
      const inner = marengoScript(s.lines, 35);
      const body = benchSessionWrapper(cfg, s.name, inner, debug);
      if (!(await step(s.name, body, 45_000))) {
        break;
      }
    }
  } else if (profile === "weighted_single_arm") {
    const angles = args.gravity_angles ?? [0, 0.3, -0.3];
    for (const a of angles) {
      const q0 = loadedJoint === "left_shoulder_pitch" ? a : 0;
      const q1 = loadedJoint === "right_shoulder_pitch" ? a : 0;
      const body = wrapRemote(
        cfg,
        `bin/motor-repl gravity-preview ${q0} ${q1}`,
        debug,
      );
      if (!(await step(`gravity_preview_${a}`, body, 30_000))) {
        return formatHarnessResult(profile, loadedJoint, steps, faults, logPath);
      }
    }

    const inner = marengoScript(
      ["home", "enable bench", "status", "gravity-on", "status", "disable", "quit"],
      40,
    );
    const body = benchSessionWrapper(cfg, "weighted_gravity_on", inner, debug);
    await step("weighted_gravity_on", body, 50_000);
  }

  // final disable + status
  await step("final_disable", wrapRemote(cfg, "bin/motor-repl disable", debug), 15_000);
  await step("final_status", wrapRemote(cfg, "bin/motor-repl status", debug), 15_000);

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
