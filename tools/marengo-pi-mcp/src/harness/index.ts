import type { BenchProfile, MarengoPiConfig } from "../config.js";
import { sudoCanUpCommand } from "../config.js";
import { MASTER_JOINTS, harnessJointSubset, profileMeta } from "../bench-profiles.js";
import {
  REFUSE_UNSETTLED_MARENGO_PI,
  canSettleShell,
  restoreCanOwnerShell,
  takeCanOwnershipShell,
} from "../can-owner.js";
import { gravityGateSnapshotShell, runGravityGate } from "../gravity-gate.js";
import { shellQuote, wrapRemoteWithConfig } from "../env.js";
import {
  REFERENCE_OPT_IN_REQUIRED,
  benchCandumpStartShell,
  benchCandumpStopShell,
  benchLogArchiveShell,
  marengoPiPipeLine,
  referenceAcquireLine,
  scriptSleepTotalSec,
} from "../tools/motion.js";
import {
  defaultPassKind,
  harnessScriptSuite,
  type HarnessPassKind,
  type HarnessScript,
} from "./scripts.js";

const ROLL_JOINT = "right_shoulder_roll";

/** Staged descent only — use when roll is elevated (q > ~0.2 rad). Do not run from home. */
export function rollStagedDescentLines(): string[] {
  return [
    `hold-at ${ROLL_JOINT} 1.0`,
    "sleep 15",
    `hold-at ${ROLL_JOINT} 0.7`,
    "sleep 15",
    `hold-at ${ROLL_JOINT} 0.4`,
    "sleep 15",
    `hold-at ${ROLL_JOINT} 0.15`,
    "sleep 12",
    `hold-at ${ROLL_JOINT} 0.05`,
    "sleep 12",
    `hold-at ${ROLL_JOINT} 0`,
    "sleep 15",
  ];
}

/** Master config dir for harness runs; optional absolute override only. */
export function harnessConfigDir(
  cfg: MarengoPiConfig,
  _profile: BenchProfile,
  configDir?: string,
): string {
  if (configDir) {
    if (configDir.startsWith("/") || configDir.startsWith("~/")) {
      return configDir;
    }
    // Legacy bringup slug — master tree only after consul-hardware-sot cutover.
    return cfg.configDir;
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
  /**
   * Aggregate smoke/heuristic success (all steps ok, no faults).
   * When pass_kind is "smoke", this does NOT mean commissioning gates (±50 mrad).
   */
  pass: boolean;
  /** smoke = fault/exit only; commissioning = metric gates evaluated. */
  pass_kind: HarnessPassKind;
  /**
   * true/false only when pass_kind is commissioning; null for smoke
   * (operator must review position-trace / candump).
   */
  commissioning_criteria_met: boolean | null;
  /** When true, do not unlock Wave/teach progression from harness alone. */
  operator_signoff_required: boolean;
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
  set_zero?: boolean;
  at_mechanical_reference?: boolean;
  debug?: boolean;
}

/** Marks where each suite starts in the single referenced session's output (stderr → $LOG). */
const SUITE_MARKER = /^=== harness suite (\S+) ===$/m;

/** marengo-pi output that means a suite failed; checked between suites to stop the session early. */
const SESSION_FAILURE =
  "control tick failed|fault=0x[0-9a-fA-F]*[1-9a-fA-F]|watchdog|outside \\[|home failed:|enable blocked:|enable failed:";

/**
 * Feeder for one marengo-pi process running every enable-requiring suite after a single awaited
 * `home <joints> sign-tested`. Grants live only in that process, so suites cannot be split across
 * processes without re-zeroing; a clean `disable` between suites keeps the grants (Davout).
 * Before each later suite the feeder stops the session (disable/quit, exit 1) if $LOG shows a failure.
 */
function referencedSessionPipe(joints: string[], scripts: HarnessScript[], timeoutSec: number): string {
  const entries = [marengoPiPipeLine(referenceAcquireLine(joints))];
  scripts.forEach((s, i) => {
    if (i > 0) {
      entries.push(
        [
          "sleep 1",
          `if grep -Eq ${shellQuote(SESSION_FAILURE)} "$LOG"; then`,
          `  echo "harness: earlier suite failed; not starting ${s.name}" >&2`,
          "  printf '%s\\n' disable quit",
          "  exit 1",
          "fi",
        ].join("\n"),
      );
    }
    entries.push(`echo "=== harness suite ${s.name} ===" >&2`);
    entries.push(...s.lines.filter((l) => l.trim() !== "quit").map(marengoPiPipeLine));
  });
  entries.push(marengoPiPipeLine("quit"));
  return `{\n${entries.join(";\n")};\n} | timeout ${timeoutSec} bin/marengo-pi`;
}

/** Sleep sum + motion/startup slack for marengo-pi pipe timeout. */
function pipeTimeoutSec(script: string[], minSec = 30): number {
  return Math.max(minSec, Math.ceil(scriptSleepTotalSec(script) + 15));
}

function benchSessionWrapper(
  cfg: MarengoPiConfig,
  profile: BenchProfile,
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
      canSettleShell(REFUSE_UNSETTLED_MARENGO_PI),
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
    harnessJointSubset(profile),
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
  const scriptSuite = harnessScriptSuite(profile);
  const passMeta: Pick<
    HarnessResult,
    "pass_kind" | "operator_signoff_required"
  > = {
    pass_kind: scriptSuite?.passKind ?? defaultPassKind(profile),
    operator_signoff_required: scriptSuite?.operatorSignoffRequired ?? false,
  };

  // Enable-requiring marengo-pi work, planned up front so a missing reference opt-in refuses
  // before anything touches the Pi.
  const sessions: HarnessScript[] =
    scriptSuite?.scripts ??
    (profile === "weighted_single_arm"
      ? [
          {
            name: "weighted_gravity_on",
            timeoutSec: 40,
            lines: ["home", "enable bench", "status", "gravity-on", "status", "disable", "quit"],
          },
        ]
      : []);
  const referenceJoints = args.joints ?? profileMeta(profile).setZeroJoints;

  const remote = (body: string) =>
    wrapRemoteWithConfig(
      cfg,
      body,
      configDir,
      debug,
      harnessJointSubset(profile),
    );

  // Harness steps are separate SSH sessions, so restore runs here rather than from an EXIT trap.
  let restoreUnit = false;
  const finish = async () => {
    if (restoreUnit) {
      await step(
        "restore_marengo_pi_service",
        remote(`MARENGO_PI_UNIT_RESTORE=true\n${restoreCanOwnerShell()}`),
        30_000,
      );
    }
    return formatHarnessResult(profile, loadedJoint, steps, faults, logPath, passMeta);
  };

  function record(name: string, out: string, ok: boolean): boolean {
    steps.push({ name, ok, output: out.slice(0, 4000) });
    if (!ok) {
      const faultLines = out
        .split("\n")
        .filter((l) =>
          /\berror\b|\bwarn\b|fault=0x[0-9a-fA-F]*[1-9a-fA-F]|watchdog|outside \[|failed:|blocked:|gravity_model_mismatch|gravity_gate_unavailable/i.test(
            l,
          ),
        );
      faults.push(...faultLines.slice(0, 20));
    }
    const logMatch = out.match(/log=(\S+)/);
    if (logMatch) logPath = logMatch[1];
    return ok;
  }

  async function step(
    name: string,
    body: string,
    timeoutMs: number,
    isOk: (out: string) => boolean = defaultStepOk,
  ): Promise<boolean> {
    const out = await runRemote(body, timeoutMs);
    return record(name, out, isOk(out));
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
    if (/home failed:|enable blocked:|enable failed:/.test(out)) {
      return false;
    }
    return true;
  }

  if (sessions.length > 0 && (args.set_zero !== true || args.at_mechanical_reference !== true)) {
    record("reference_required", REFERENCE_OPT_IN_REQUIRED, false);
    return finish();
  }

  // 1. health, sole CAN ownership (stops marengo-pi.service; restored in finish), can up
  const healthBody = remote(
    [
      "ip -br link show type can || true",
      "test -x bin/marengo-pi",
      "cat .deploy-rev 2>/dev/null || true",
    ].join("\n"),
  );
  if (!(await step("health", healthBody, 30_000))) {
    return finish();
  }

  // Gravity-gate measured torque must be read before take_can_ownership stops marengo-pi.
  const gateSnapshot = await runRemote(remote(gravityGateSnapshotShell()), 15_000);

  const tookCan = await step("take_can_ownership", remote(takeCanOwnershipShell()), 30_000);
  restoreUnit = /^marengo-pi\.service restore after session: true$/m.test(
    steps[steps.length - 1].output,
  );
  if (!tookCan) {
    return finish();
  }

  const canUpBody = remote(sudoCanUpCommand(cfg));
  if (!(await step("can_up", canUpBody, 60_000))) {
    return finish();
  }

  // 2. motor-repl status
  if (!(await step("motor_repl_status", remote("bin/motor-repl status"), 30_000))) {
    return finish();
  }

  // 3. gravity-model gate (every profile) before any enable
  const gate = await runGravityGate({
    profile,
    joints: referenceJoints,
    snapshotOutput: gateSnapshot,
    runPreview: (shell) => runRemote(remote(shell), 30_000),
  });
  if (!record("gravity_gate", gate.report, gate.ok)) {
    return finish();
  }

  if (scriptSuite?.note) {
    record(scriptSuite.note.name, scriptSuite.note.output, true);
  }
  if (profile === "weighted_single_arm") {
    const angles = args.gravity_angles ?? [0, 0.3, -0.3];
    for (const a of angles) {
      const pose = MASTER_JOINTS.map((joint) => (joint === loadedJoint ? a : 0));
      const body = remote(`bin/motor-repl gravity-preview ${pose.join(" ")}`);
      if (!(await step(`gravity_preview_${a}`, body, 30_000))) {
        return finish();
      }
    }
  }

  if (sessions.length > 0) {
    const pipeSec =
      sessions.reduce((sum, s) => sum + pipeTimeoutSec(s.lines, s.timeoutSec), 0) +
      scriptSleepTotalSec([referenceAcquireLine(referenceJoints)]);
    const out = await runRemote(
      benchSessionWrapper(
        cfg,
        profile,
        configDir,
        "referenced_session",
        referencedSessionPipe(referenceJoints, sessions, pipeSec),
        debug,
      ),
      (pipeSec + 30) * 1000,
    );
    // split() with a capture group yields [beforeFirstSuite, name1, output1, name2, output2, ...].
    const [acquireOut, ...suiteParts] = out.split(SUITE_MARKER);
    record(
      "reference_acquire",
      acquireOut,
      referenceJoints.every((j) => new RegExp(`^reference ${j} current `, "m").test(acquireOut)) &&
        defaultStepOk(acquireOut),
    );
    for (const s of sessions) {
      const at = suiteParts.indexOf(s.name);
      if (at % 2 === 0) {
        record(s.name, suiteParts[at + 1] ?? "", defaultStepOk(suiteParts[at + 1] ?? ""));
      } else {
        record(s.name, "not run: the referenced session stopped before this suite", false);
      }
    }
  }

  // final disable + status
  await step("final_disable", remote("bin/motor-repl disable"), 15_000);
  await step("final_status", remote("bin/motor-repl status"), 15_000);

  return finish();
}

function formatHarnessResult(
  profile: BenchProfile,
  loadedJoint: string | undefined,
  steps: HarnessStep[],
  faults: string[],
  logPath: string | undefined,
  passMeta: Pick<HarnessResult, "pass_kind" | "operator_signoff_required">,
): string {
  const pass = steps.every((s) => s.ok) && faults.length === 0;
  const result: HarnessResult = {
    profile,
    pass,
    pass_kind: passMeta.pass_kind,
    commissioning_criteria_met:
      passMeta.pass_kind === "commissioning" ? pass : null,
    operator_signoff_required: passMeta.operator_signoff_required,
    loaded_joint: loadedJoint,
    steps: steps.map((s) => ({ name: s.name, ok: s.ok, output: s.output.slice(0, 500) })),
    faults,
    log_path: logPath,
  };

  const summary = JSON.stringify(result, null, 2);
  const kindLabel =
    passMeta.pass_kind === "smoke"
      ? "SMOKE"
      : "COMMISSIONING";
  const human = [
    `pass_kind=${passMeta.pass_kind} (${kindLabel}${
      passMeta.operator_signoff_required ? "; operator sign-off still required" : ""
    })`,
    ...steps.map((s) => `[${s.ok ? "PASS" : "FAIL"}] ${s.name}`),
  ].join("\n");

  return `${human}\n\n--- JSON ---\n${summary}`;
}
