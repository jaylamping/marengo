import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import type { MarengoPiConfig } from "../src/config.js";
import { MASTER_JOINTS } from "../src/bench-profiles.js";
import type { CalibrationDeps } from "../src/tools/calibration-session.js";
import {
  type JointCalPlan,
  type JointCalibrateArgs,
  type JointLimits,
  LIMIT_INSET_RAD,
  MAX_WAVE_AMPLITUDE_RAD,
  MIN_WAVE_AMPLITUDE_RAD,
  MIN_WAVE_PEAK_SPEED_RAD_S,
  SPEED_CAP_SHARE,
  TAU_SAMPLE_STEP_RAD,
  checkJointCalLimits,
  checkWavePoseSpacing,
  checkWaveSpeeds,
  defaultPoses,
  defaultVelocityPasses,
  deriveWaveAmplitude,
  gravityBatchShell,
  guardConfigurations,
  jointCalibrationScript,
  localWaves,
  maxSweepTauAtPoses,
  parseGravityBatch,
  planJointCalibration,
  readJointLimits,
  registerJointCalibrateTools,
  stepsSeenInTrace,
  wavePeakSpeed,
} from "../src/tools/joint-calibrate.js";
import { MAX_SESSION_SLEEP_SEC } from "../src/tools/calibration-session.js";
import { scriptSleepTotalSec } from "../src/tools/motion.js";
import { gravityPreviewReply, isGravityPreviewBody } from "./gravity-fixture.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

const REPO = new URL("../../../", import.meta.url);
const repoFile = (rel: string) => readFileSync(new URL(rel, REPO), "utf8");
const FILES = {
  robotYaml: repoFile("config/robot.yaml"),
  controlYaml: repoFile("config/control.yaml"),
  motorsYaml: repoFile("config/motors.yaml"),
  urdf: repoFile("assets/urdf/marengo.urdf"),
};
const [PITCH, , YAW, ELBOW] = MASTER_JOINTS;
const CHAIN = [...MASTER_JOINTS];
const TS = "20261004T120000Z";
const TRACE_PATH = `/opt/marengo/var/log/position-trace-${TS}.csv`;
const CAL_DIR = `/tmp/marengo/var/gravity-calibration/${TS}`;
const SESSION_JSON = `{"log":"/opt/marengo/var/log/bench-${TS}.log","trace":"${TRACE_PATH}","candump":"","ts":"${TS}","label":"joint-calibrate"}`;
const OPT_INS = {
  confirm_weighted_motion: true as const,
  set_zero: true,
  at_mechanical_reference: true,
  skip_hanging_rest_gravity_check: true,
};

function repoLimits(): Record<string, JointLimits> {
  const read = readJointLimits(FILES, CHAIN);
  assert.ok(read.ok, read.ok ? "" : read.message);
  return read.limits;
}

function marked(name: string, content: string): string {
  return `=====MARENGO_GRAVCAL_${name}_BEGIN=====\n${content}\n=====MARENGO_GRAVCAL_${name}_END=====`;
}

function preflightReply(control = FILES.controlYaml): string {
  return [
    marked("robot", FILES.robotYaml),
    marked("control", control),
    marked("motors", FILES.motorsYaml),
    "MARENGO_GRAVCAL_URDF_PATH=/opt/marengo/assets/urdf/marengo.urdf",
    marked("urdf", FILES.urdf),
  ].join("\n");
}

// ---------------------------------------------------------------------------
// Synthetic position trace (Berthier header; `phase` quoted with a comma inside).

const TRACE_HEADER =
  "tick,t_ms,joint,q,dq,q_traj,dq_traj,q_des,target,target_raw,q_env_lo,q_env_hi,lead,lead_sat,settle_error,phase,friction_mode,tau_p,tau_g,tau_f,tau_d,tau_ff_cmd,tau_meas,dq_mit,kp,kd,joint_stuck,planner_frozen,retarget_age_ms,planner_event,law,q_ref,dq_ref,time_scale,tau_i,kd_mit,tau_ff_wire";
const TRACE_HZ = 50;

/** Trace rows the stdin motion lines (`hold-at`, `wave`) of a session body would leave. */
function traceFor(motionLines: readonly string[]): string {
  const targets: Record<string, number> = Object.fromEntries(CHAIN.map((j) => [j, 0.0012]));
  const rows = [TRACE_HEADER];
  let tick = 0;
  const emit = () => {
    for (const j of CHAIN) {
      const t = targets[j].toFixed(6);
      rows.push(`${tick},${tick * 20},${j},${t},0,${t},0,${t},${t},${t},-1,3,0,false,0,"Hold, settled",static,0,0,0,0,0,0,0,18,3,false,false,0,tick,legacy,${t},0.000000,1.000000,0.000000,3.000,0.000000`);
    }
    tick += 1;
  };
  emit();
  for (const line of motionLines) {
    const [verb, joint, ...nums] = line.split(" ");
    if (verb === "hold-at") {
      targets[joint] = Number(nums[0]);
      for (let k = 0; k < 10; k += 1) emit();
    } else if (verb === "wave") {
      const [min, max, cycles, half] = nums.map(Number);
      const n = Math.round(cycles * 2 * half * TRACE_HZ);
      for (let k = 0; k <= n; k += 1) {
        targets[joint] = min + ((max - min) * (1 - Math.cos((Math.PI * k) / (half * TRACE_HZ)))) / 2;
        emit();
      }
    }
  }
  return `${rows.join("\n")}\n`;
}

const stdinLines = (body: string) => [...body.matchAll(/printf '%s\\n' "([^"]+)"/g)].map((m) => m[1]);
const motionLines = (body: string) => stdinLines(body).filter((l) => /^(hold-at|wave) /.test(l));

// ---------------------------------------------------------------------------
// Tool harness

type TauModel = (q: Record<string, number>) => Record<string, number>;

interface Harness {
  bodies: string[];
  writes: Map<string, string>;
  fits: string[][];
  audits: { tool: string; exitCode: number }[];
  run: (args: Partial<JointCalibrateArgs>) => Promise<string>;
}

function harness(
  opts: {
    preflight?: string;
    tau?: TauModel;
    sessionExit?: number;
    /** Keep only this many motion lines in the synthetic trace (an aborted session). */
    traceMotionLines?: number;
  } = {},
): Harness {
  const h: Harness = { bodies: [], writes: new Map(), fits: [], audits: [], run: async () => "" };
  let session = "";
  const runRemote = async (body: string) => {
    h.bodies.push(body);
    if (body.includes("MARENGO_GRAVCAL_URDF_PATH=")) return opts.preflight ?? preflightReply();
    if (body.includes("@@gravcal_pose")) {
      const rows = /printf '%s\\n' ((?:'[^']*' ?)+) \| while/.exec(body)?.[1] ?? "";
      return [...rows.matchAll(/'([^']*)'/g)]
        .map((m) => {
          const [index, ...values] = m[1].split(" ").map(Number);
          const q = Object.fromEntries(CHAIN.map((j, i) => [j, values[i]]));
          return `@@gravcal_pose ${index}\n${gravityPreviewReply(opts.tau?.(q) ?? {})}`;
        })
        .join("\n");
    }
    if (isGravityPreviewBody(body)) return gravityPreviewReply();
    if (body.includes("pi_now_ms=")) return "";
    if (body.includes("LABEL='joint-calibrate'")) {
      session = body;
      const exit = opts.sessionExit ? `\n[exit ${opts.sessionExit}]` : "";
      return `=== bench session ${TS} (joint-calibrate) ===\nstatus ok\n${SESSION_JSON}${exit}`;
    }
    if (body.includes("MARENGO_GRAVCAL_trace_BEGIN")) {
      return marked("trace", traceFor(motionLines(session).slice(0, opts.traceMotionLines)));
    }
    throw new Error(`unexpected remote body:\n${body.slice(0, 400)}`);
  };
  const deps: CalibrationDeps = {
    execLocal: async (_command, args) => {
      h.fits.push(args);
      return { stdout: "proposal written", stderr: "", exitCode: 0 };
    },
    writeFile: async (file, data) => {
      h.writes.set(file, data);
    },
    mkdir: async () => {},
    now: () => new Date("2026-10-04T12:00:00.000Z"),
  };
  const tools = registerJointCalibrateTools(cfg, runRemote, (tool, _a, _o, exitCode) => h.audits.push({ tool, exitCode }), deps);
  h.run = (args) => tools.pi_joint_calibrate.handler({ confirm: true, ...args } as JointCalibrateArgs);
  return h;
}

function plan(overrides: Partial<Parameters<typeof planJointCalibration>[0]> = {}): JointCalPlan {
  const p = planJointCalibration({
    sweepJoint: ELBOW,
    chain: CHAIN,
    limits: repoLimits(),
    fixedRad: {},
    amplitudeFraction: 0.25,
    approachOffsetRad: 0.05,
    method: "static",
    ...overrides,
  });
  assert.ok(p.ok, p.ok ? "" : p.message);
  return p;
}

// ---------------------------------------------------------------------------

describe("pi_joint_calibrate plan builder", () => {
  it("reads every master joint's window, τ_ff cap and velocity cap from the repo config", () => {
    const limits = repoLimits();
    // URDF effort ∩ motors.yaml torque_limit ∩ robot max_joint_torque ∩ motor-type tau_ff_max_nm.
    assert.equal(limits[PITCH].tauFfCapNm, 5);
    assert.equal(limits[ELBOW].tauFfCapNm, 3);
    // Joint > actuator group > motor type.
    assert.equal(limits[PITCH].velocityCapRadS, 2.5);
    assert.equal(limits[ELBOW].velocityCapRadS, 1.5);
    assert.equal(limits[ELBOW].trajectoryAccelRadS2, 2.5);
  });

  for (const fraction of [0.25, 0.5, 0.9]) {
    it(`default poses span ${fraction} of the window, centred on 0, inside the inset`, () => {
      for (const joint of CHAIN) {
        const w = repoLimits()[joint].window;
        const delta = 0.05;
        const r = defaultPoses(w, fraction, delta);
        assert.ok(r.ok);
        const { poses } = r;
        assert.equal(poses.length, 5);
        const lo = w.lower + LIMIT_INSET_RAD + delta;
        const hi = w.upper - LIMIT_INSET_RAD - delta;
        assert.ok(poses[0] >= lo && poses[4] <= hi, `${joint} ${poses} inside [${lo}, ${hi}]`);
        const span = poses[4] - poses[0];
        assert.ok(Math.abs(span - Math.min(fraction * (w.upper - w.lower), hi - lo)) <= 0.003, `${joint} span ${span}`);
        const gaps = poses.slice(1).map((q, i) => q - poses[i]);
        assert.ok(Math.max(...gaps) - Math.min(...gaps) <= 0.002, `${joint} evenly spaced: ${gaps}`);
        if (fraction * (w.upper - w.lower) < hi - lo) {
          // Centred on the reference when the inset allows, else shifted against the nearer limit.
          const mid = (poses[0] + poses[4]) / 2;
          assert.ok(Math.abs(mid) <= 0.002 || poses[0] - lo <= 0.001 || hi - poses[4] <= 0.001, `${joint} mid ${mid}`);
        }
      }
    });
  }

  it("every target, overshoot and wave extreme of a default plan clears the 0.05 rad inset", () => {
    const limits = repoLimits();
    for (const joint of CHAIN) {
      for (const amplitudeFraction of [0.25, 0.5, 0.9]) {
        const p = plan({ sweepJoint: joint, amplitudeFraction });
        assert.deepEqual(checkJointCalLimits(p, limits), { ok: true }, `${joint} ${amplitudeFraction}`);
        assert.deepEqual(checkWaveSpeeds(p, limits), { ok: true }, `${joint} ${amplitudeFraction}`);
      }
    }
  });

  it("orders fixed poses, both static passes, then the waves", () => {
    const p = plan({ fixedRad: { [PITCH]: 0.5, [YAW]: 0 } });
    assert.deepEqual(p.fixedRad, { [PITCH]: 0.5 }, "a zero fixed pose means stay at the reference");
    assert.deepEqual(p.steps[0], { kind: "fixed", joint: PITCH, target_rad: 0.5 });
    const holds = p.steps.filter((s) => s.kind === "hold" && s.measure);
    assert.equal(holds.length, 10);
    assert.deepEqual(
      holds.map((s) => s.kind === "hold" && s.approach),
      [...Array(5).fill("below"), ...Array(5).fill("above")],
    );
    const waves = p.steps.filter((s) => s.kind === "wave");
    assert.equal(waves.length, 3);
    const firstWave = p.steps.findIndex((s) => s.kind === "wave");
    assert.ok(p.steps.slice(firstWave).every((s) => s.kind === "wave"));
    const prePosition = p.steps[firstWave - 1];
    assert.ok(prePosition.kind === "hold" && waves[0].kind === "wave");
    assert.equal(prePosition.target_rad, waves[0].min_rad, "the joint is parked at the wave's start first");
  });

  it("wave peak speed is π·(max−min)/(2·half_period); defaults stay within 80 % of the velocity cap", () => {
    assert.ok(Math.abs(wavePeakSpeed(0, 0.3, 1) - 0.15 * Math.PI) < 1e-12);
    const limits = repoLimits();
    for (const joint of CHAIN) {
      const p = plan({ sweepJoint: joint });
      const passes = defaultVelocityPasses(p.posesRad, limits[joint]);
      assert.ok(passes.max_rad - passes.min_rad <= 0.3 + 1e-9);
      assert.ok(passes.min_rad >= p.posesRad[0] && passes.max_rad <= p.posesRad[4]);
      const speeds = passes.half_periods_s.map((t) => wavePeakSpeed(passes.min_rad, passes.max_rad, t));
      assert.equal(speeds.length, 3);
      assert.ok(speeds[0] < speeds[1] && speeds[1] < speeds[2], `${joint} three ascending speeds ${speeds}`);
      assert.ok(speeds[2] <= SPEED_CAP_SHARE * limits[joint].velocityCapRadS, `${joint} ${speeds[2]}`);
    }
  });

  it("default sessions fit the 300 s budget; long dwells with many poses do not", () => {
    for (const joint of CHAIN) {
      const script = jointCalibrationScript(plan({ sweepJoint: joint }), {
        operator: "bench",
        settleSec: 2.5,
        measureSec: 1.5,
        returnHomeSec: 6,
      });
      assert.ok(scriptSleepTotalSec(script) <= MAX_SESSION_SLEEP_SEC, `${joint} ${scriptSleepTotalSec(script)}`);
    }
    const long = jointCalibrationScript(plan({ posesRad: [0, 0.05, 0.1, 0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.45] }), {
      operator: "bench",
      settleSec: 10,
      measureSec: 5,
      returnHomeSec: 6,
    });
    assert.ok(scriptSleepTotalSec(long) > MAX_SESSION_SLEEP_SEC);
  });

  it("script: reference all joints, holds, waves, then every joint to 0 distal first", () => {
    const p = plan({ fixedRad: { [PITCH]: 0.5 } });
    const script = jointCalibrationScript(p, { operator: "bench", settleSec: 2.5, measureSec: 1.5, returnHomeSec: 6 });
    assert.deepEqual(script.slice(0, 5), [
      `home ${CHAIN.join(" ")} sign-tested`,
      "home",
      "enable bench",
      `hold-at ${PITCH} 0.5`,
      "sleep 4",
    ]);
    const wave = p.steps.find((s) => s.kind === "wave");
    assert.ok(wave?.kind === "wave");
    assert.ok(
      script.includes(`wave ${ELBOW} ${wave.min_rad} ${wave.max_rad} 2 ${wave.half_period_s}`),
      "issued as marengo-pi's own wave command",
    );
    const tail = script.slice(script.indexOf(`hold-at ${CHAIN.at(-1)} 0`));
    assert.deepEqual(tail, [
      "hold-at right_lower_arm_yaw 0",
      "hold-at right_elbow_pitch 0",
      "sleep 6",
      "hold-at right_upper_arm_yaw 0",
      "hold-at right_shoulder_roll 0",
      "hold-at right_shoulder_pitch 0",
      "sleep 6",
      "status",
      "disable",
      "quit",
    ]);
  });

  it("τ guard samples every commanded path at most 0.05 rad apart", () => {
    const p = plan({ fixedRad: { [PITCH]: 0.5 } });
    const configs = guardConfigurations(p);
    const fixedPath = configs.filter((c) => (c[ELBOW] ?? 0) === 0).map((c) => c[PITCH] ?? 0).sort((a, b) => a - b);
    assert.equal(fixedPath[0], 0);
    assert.equal(fixedPath.at(-1), 0.5);
    const sweep = configs.filter((c) => c[PITCH] === 0.5 && c[ELBOW] !== undefined).map((c) => c[ELBOW]).sort((a, b) => a - b);
    const targets = p.steps.flatMap((s) => (s.kind === "wave" ? [s.min_rad, s.max_rad] : s.joint === ELBOW ? [s.target_rad] : []));
    assert.ok(sweep[0] <= Math.min(0, ...targets) && sweep.at(-1)! >= Math.max(...targets));
    for (const t of targets) assert.ok(sweep.includes(t), `exact target ${t} sampled`);
    for (let i = 1; i < sweep.length; i += 1) assert.ok(sweep[i] - sweep[i - 1] <= TAU_SAMPLE_STEP_RAD + 1e-9);
  });

  it("gravity batch shell runs one full-vector preview per configuration and parses back", () => {
    const dir = mkdtempSync(path.join(tmpdir(), "jointcal-"));
    mkdirSync(path.join(dir, "bin"));
    writeFileSync(
      path.join(dir, "bin/motor-repl"),
      '#!/bin/bash\n[[ "$1" == gravity-preview && $# -eq 6 ]] || exit 3\necho "right_shoulder_pitch: tau_g = $2 Nm"\necho "right_elbow_pitch: tau_g = $5 Nm"\n',
    );
    chmodSync(path.join(dir, "bin/motor-repl"), 0o755);
    const configs: Record<string, number>[] = [{}, { [PITCH]: 0.25 }, { [PITCH]: 0.25, [ELBOW]: -0.5 }];
    const r = spawnSync("bash", ["-c", `set -euo pipefail\n${gravityBatchShell(configs, CHAIN)}`], { cwd: dir, encoding: "utf8" });
    assert.equal(r.status, 0, r.stderr);
    const tau = parseGravityBatch(r.stdout);
    assert.deepEqual([...tau.entries()], [
      [0, { [PITCH]: 0, [ELBOW]: 0 }],
      [1, { [PITCH]: 0.25, [ELBOW]: 0 }],
      [2, { [PITCH]: 0.25, [ELBOW]: -0.5 }],
    ]);
  });

  it("counts the steps a trace reached, waves included", () => {
    const p = plan();
    const script = jointCalibrationScript(p, { operator: "bench", settleSec: 2.5, measureSec: 1.5, returnHomeSec: 6 });
    const motion = script.filter((l) => /^(hold-at|wave) /.test(l));
    assert.equal(stepsSeenInTrace(p.steps, traceFor(motion)), p.steps.length);
    const lastWave = motion.map((l) => l.startsWith("wave ")).lastIndexOf(true);
    const cut = motion.slice(0, lastWave);
    assert.equal(stepsSeenInTrace(p.steps, traceFor(cut)), p.steps.length - 1);
    const halfWave = traceFor(motion.slice(0, lastWave + 1)).split("\n");
    assert.equal(
      stepsSeenInTrace(p.steps, halfWave.slice(0, Math.floor(halfWave.length * 0.95)).join("\n")),
      p.steps.length - 1,
      "a wave cut before its last return to min is not complete",
    );
  });
});

describe("pi_joint_calibrate refusals", () => {
  it("refuses before touching the Pi without opt-ins or with joints outside the profile", async () => {
    const cases: [Partial<JointCalibrateArgs>, RegExp][] = [
      [{ ...OPT_INS, sweep_joint: ELBOW, confirm_weighted_motion: undefined }, /^Weighted motion blocked/],
      [{ ...OPT_INS, sweep_joint: ELBOW, set_zero: false }, /^Refused: enabling needs a current reference/],
      [{ ...OPT_INS, sweep_joint: ELBOW, skip_hanging_rest_gravity_check: false }, /pi_joint_calibrate needs skip_hanging_rest_gravity_check: true/],
      [{ ...OPT_INS, sweep_joint: ELBOW, profile: "roll_attached" }, /roll_attached does not reference right_elbow_pitch/],
      [{ ...OPT_INS, sweep_joint: ELBOW, operator: "a b" }, /operator must match/],
    ];
    for (const [args, pattern] of cases) {
      const h = harness();
      assert.match(await h.run(args), pattern);
      assert.equal(h.bodies.length, 0);
    }
  });

  it("limit: a pose within 0.05 rad of the window refuses after the read-only pre-flight", async () => {
    const upper = repoLimits()[ELBOW].window.upper;
    const h = harness();
    const out = await h.run({ ...OPT_INS, method: "static", sweep_joint: ELBOW, poses_rad: [0.1, roundTo3(upper - 0.06)] });
    assert.match(out, /right_elbow_pitch step \d+ target .* is outside its allowed window .* shrunk by 0\.05 rad on each side\); no motion was run\./);
    assert.equal(h.bodies.length, 1, "only the pre-flight read ran");
    assert.deepEqual(h.audits, [{ tool: "pi_joint_calibrate", exitCode: 1 }]);
  });

  it("limit: a fixed pose or wave extreme inside the window but within the inset refuses", () => {
    const limits = repoLimits();
    const pitchLo = limits[PITCH].window.lower;
    const fixed = checkJointCalLimits(plan({ fixedRad: { [PITCH]: roundTo3(pitchLo + 0.02) } }), limits);
    assert.ok(!fixed.ok && /right_shoulder_pitch step 0 fixed pose/.test(fixed.message));
    const elbowHi = limits[ELBOW].window.upper;
    const wave = checkJointCalLimits(
      plan({ velocityPasses: { min_rad: 1.0, max_rad: roundTo3(elbowHi - 0.01), half_periods_s: [3], cycles: 1 } }),
      limits,
    );
    assert.ok(!wave.ok && /right_elbow_pitch step \d+ wave max/.test(wave.message));
  });

  it("τ × 1.6: model gravity over 80 % of the τ_ff cap refuses before the gate or any motion", async () => {
    // Pitch cap 5 Nm: 2.6 Nm × 1.6 = 4.16 Nm > 4.0 Nm.
    const h = harness({ tau: (q) => ({ [PITCH]: (q[PITCH] ?? 0) > 0.3 ? 2.6 : 0.1 }) });
    const out = await h.run({ ...OPT_INS, sweep_joint: PITCH });
    assert.match(out, /^Refused: τ guard: right_shoulder_pitch: max \|τ_g\| 2\.600 Nm at right_shoulder_pitch=0\.\d+; × 1\.6 = 4\.160 Nm vs 0\.8 × cap 5 Nm = 4\.000 Nm\./);
    assert.match(out, /No motion was run\.$/);
    assert.equal(h.bodies.length, 2, "pre-flight and the τ_g batch only");
    assert.match(h.bodies[1], /@@gravcal_pose/);
    assert.match(h.bodies[1], /sudo -n .* stop/, "the batch runs as sole CAN owner, like the gate");
    assert.equal(h.writes.size, 0);
  });

  it("τ guard fails closed when gravity-preview prints nothing", async () => {
    const bodies: string[] = [];
    const tools = registerJointCalibrateTools(
      cfg,
      async (body) => {
        bodies.push(body);
        return body.includes("MARENGO_GRAVCAL_URDF_PATH=") ? preflightReply() : "[exit 1]";
      },
      () => {},
    );
    const out = await tools.pi_joint_calibrate.handler({ confirm: true, ...OPT_INS, sweep_joint: ELBOW });
    assert.match(out, /^Refused: τ guard unavailable: gravity-preview gave no τ_g for right_shoulder_pitch/);
    assert.equal(bodies.length, 2);
  });

  it("speed: a wave above 80 % of the velocity cap refuses after the pre-flight", async () => {
    // Elbow cap 1.5 rad/s (actuator group): π·0.6/(2·0.6) = 1.571 rad/s > 1.2.
    const h = harness();
    const out = await h.run({
      ...OPT_INS,
      method: "static",
      sweep_joint: ELBOW,
      velocity_passes: { min_rad: 0.1, max_rad: 0.7, half_periods_s: [0.6], cycles: 1 },
    });
    assert.match(out, /^Refused: wave right_elbow_pitch \[0\.1, 0\.7\] half period 0\.6 s peaks at 1\.571 rad\/s, above 80 % of its velocity cap 1\.5 rad\/s \(1\.200 rad\/s\)/);
    assert.equal(h.bodies.length, 1);
  });

  it("budget: over 300 s refuses with a suggestion to split", async () => {
    const h = harness();
    const out = await h.run({
      ...OPT_INS,
      method: "static",
      sweep_joint: ELBOW,
      poses_rad: [0, 0.05, 0.1, 0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.45],
      settle_sec: 10,
      measure_sec: 5,
    });
    assert.match(out, /^Refused: session budget \d+(\.\d+)? s .* exceeds 300 s\. Split it into several pi_joint_calibrate sessions/);
    assert.equal(h.bodies.length, 1);
  });
});

describe("pi_joint_calibrate session and v2 plan.json", () => {
  it("runs one session and writes plan.json version 2 with session_complete true", async () => {
    const h = harness();
    const out = await h.run({ ...OPT_INS, method: "static", sweep_joint: ELBOW, fixed_rad: { [PITCH]: 0.5 } });
    assert.equal(h.bodies.length, 6, "pre-flight, τ batch, gate snapshot, gate preview, session, trace");
    const session = h.bodies[4];
    const lines = stdinLines(session);
    assert.equal(lines[0], `home ${CHAIN.join(" ")} sign-tested`);
    assert.equal(lines[3], `hold-at ${PITCH} 0.5`);
    assert.ok(lines.some((l) => l.startsWith(`wave ${ELBOW} `)));

    const planJson = JSON.parse(h.writes.get(`${CAL_DIR}/plan.json`) ?? "{}");
    assert.deepEqual(Object.keys(planJson), [
      "version",
      "created_utc",
      "session_ts",
      "profile",
      "method",
      "sweep_joint",
      "fixed_rad",
      "poses_rad",
      "approach_offset_rad",
      "settle_sec",
      "measure_sec",
      "steps",
      "gravity_gate_report",
      "session_complete",
    ]);
    assert.equal(planJson.version, 2);
    assert.equal(planJson.profile, "arm_attached");
    assert.equal(planJson.sweep_joint, ELBOW);
    assert.deepEqual(planJson.fixed_rad, { [PITCH]: 0.5 });
    assert.equal(planJson.session_complete, true);
    const kinds = new Set(planJson.steps.map((s: { kind: string }) => s.kind));
    assert.deepEqual([...kinds], ["fixed", "hold", "wave"]);
    for (const s of planJson.steps) {
      if (s.kind === "wave") {
        assert.deepEqual(Object.keys(s).sort(), ["cycles", "half_period_s", "joint", "kind", "max_rad", "min_rad"]);
      } else if (s.kind === "fixed") {
        assert.deepEqual(Object.keys(s).sort(), ["joint", "kind", "target_rad"]);
      } else {
        assert.equal(typeof s.measure, "boolean");
      }
    }
    // Every hold-at / wave stdin line is a plan step, in order (then the return to 0).
    const motion = motionLines(session);
    const expected = planJson.steps.map((s: Record<string, number | string>) =>
      s.kind === "wave"
        ? `wave ${s.joint} ${s.min_rad} ${s.max_rad} ${s.cycles} ${s.half_period_s}`
        : `hold-at ${s.joint} ${s.target_rad}`,
    );
    assert.deepEqual(motion.slice(0, expected.length), expected);
    assert.ok(h.writes.has(`${CAL_DIR}/position-trace.csv`));
    assert.ok(h.writes.has(`${CAL_DIR}/pi-marengo.urdf`));
    assert.match(h.writes.get(`${CAL_DIR}/bench-session.txt`) ?? "", /^τ guard: \d+ configurations/);
    assert.equal(h.fits.length, 1);
    assert.deepEqual(h.fits[0].slice(6, 9), ["gravity-fit", "--dir", CAL_DIR]);
    assert.match(out, /gravity-fit: proposal written/);
    assert.deepEqual(h.audits, [{ tool: "pi_joint_calibrate", exitCode: 0 }]);
  });

  it("session_complete is false when marengo-pi exits non-zero; the fit still uses the completed steps", async () => {
    const h = harness({ sessionExit: 1 });
    const out = await h.run({ ...OPT_INS, sweep_joint: ELBOW });
    const planJson = JSON.parse(h.writes.get(`${CAL_DIR}/plan.json`) ?? "{}");
    assert.equal(planJson.session_complete, false);
    assert.match(out, /session_complete is false; the fitter uses completed steps only/);
    assert.equal(h.fits.length, 1);
    assert.deepEqual(h.audits, [{ tool: "pi_joint_calibrate", exitCode: 1 }]);
  });

  it("session_complete is false when a planned step never reached the trace", async () => {
    const h = harness({ traceMotionLines: 6 });
    await h.run({ ...OPT_INS, sweep_joint: ELBOW });
    const planJson = JSON.parse(h.writes.get(`${CAL_DIR}/plan.json`) ?? "{}");
    assert.equal(planJson.session_complete, false);
    assert.deepEqual(h.audits, [{ tool: "pi_joint_calibrate", exitCode: 1 }]);
  });
});

describe("pi_joint_calibrate wave method", () => {
  const wavePlan = (joint: string, amplitudeRad: number, overrides: Partial<Parameters<typeof planJointCalibration>[0]> = {}) => {
    const limits = repoLimits();
    const waves = localWaves(joint, limits[joint], amplitudeRad, 200);
    assert.ok(waves.ok, waves.ok ? "" : waves.message);
    return plan({ sweepJoint: joint, method: "wave", waves: waves.waves, ...overrides });
  };

  it("reads kp, the friction breakaway and the loop rate from control.yaml", () => {
    const read = readJointLimits(FILES, CHAIN);
    assert.ok(read.ok);
    assert.equal(read.loopHz, 200);
    assert.equal(read.limits[PITCH].kp, 18);
    assert.equal(read.limits[PITCH].breakawayNm, 0.08, "no fs: Coulomb fc");
    assert.equal(read.limits[ELBOW].kp, 12);
  });

  it("amplitude = (F_s + 0.6 × max|τ_g|)/kp, at least the floor, refused above the cap", () => {
    const pitch = repoLimits()[PITCH];
    const derived = deriveWaveAmplitude(PITCH, pitch, 1.4);
    assert.ok(derived.ok);
    // (0.08 + 0.6 × 1.4) / 18 = 0.05111 → 0.052 (up to 1 mrad).
    assert.equal(derived.amplitudeRad, 0.052);
    assert.match(derived.basis, /F_s 0\.08 \+ 0\.6 × max\|τ_g\| 1\.400 Nm\) \/ kp 18/);
    const floor = deriveWaveAmplitude(PITCH, pitch, 0);
    assert.ok(floor.ok && floor.amplitudeRad === MIN_WAVE_AMPLITUDE_RAD);
    const big = deriveWaveAmplitude(PITCH, pitch, 3);
    assert.ok(!big.ok && /exceeds 0\.1 rad/.test(big.message));
    const noKp = deriveWaveAmplitude(PITCH, { ...pitch, kp: undefined }, 1);
    assert.ok(!noKp.ok && /impedance\.kp/.test(noKp.message));
  });

  it("speeds run from 0.2 rad/s to the admissible speed, with cycles enough for the centre bin", () => {
    const limits = repoLimits();
    for (const joint of CHAIN) {
      for (const a of [MIN_WAVE_AMPLITUDE_RAD, 0.05, MAX_WAVE_AMPLITUDE_RAD]) {
        const r = localWaves(joint, limits[joint], a, 200);
        assert.ok(r.ok, r.ok ? "" : r.message);
        const speeds = r.waves.passes.map((p) => wavePeakSpeed(-a, a, p.half_period_s));
        assert.ok(speeds.length >= 2 && speeds.length <= 3, `${joint} ${a}: ${speeds}`);
        assert.ok(speeds[0] >= MIN_WAVE_PEAK_SPEED_RAD_S, `${joint} ${a}: slowest ${speeds[0]}`);
        for (const [i, p] of r.waves.passes.entries()) {
          // Centre bin a/2 wide at 200 Hz: samples per direction ≥ 1.5 × 10.
          assert.ok((p.cycles * (0.5 * a * 200)) / speeds[i] >= 15, `${joint} ${a} ${speeds[i]} × ${p.cycles}`);
        }
        const p = wavePlan(joint, a);
        assert.deepEqual(checkWaveSpeeds(p, limits), { ok: true }, `${joint} ${a}`);
      }
    }
    const slow = localWaves(PITCH, limits[PITCH], 0.05, 200, [0.1, 0.3]);
    assert.ok(!slow.ok && /below 0\.2 rad\/s/.test(slow.message));
  });

  it("plans a park and one local wave per speed at every pose, centred on the pose", () => {
    const limits = repoLimits();
    for (const joint of CHAIN) {
      const p = wavePlan(joint, MAX_WAVE_AMPLITUDE_RAD);
      assert.equal(p.method, "wave");
      assert.equal(p.waveAmplitudeRad, MAX_WAVE_AMPLITUDE_RAD);
      assert.deepEqual(checkJointCalLimits(p, limits), { ok: true }, `${joint}: default poses leave room for the cap`);
      assert.equal(p.steps.filter((s) => s.kind === "hold" && s.measure).length, 0, "no static holds");
      p.posesRad.forEach((pose, i) => {
        const waves = p.steps.filter((s) => s.kind === "wave" && s.pose_index === i);
        assert.ok(waves.length >= 2, `${joint} pose ${i}`);
        for (const w of waves) {
          assert.ok(w.kind === "wave");
          assert.ok(Math.abs((w.min_rad + w.max_rad) / 2 - pose) < 1e-9);
          assert.ok(Math.abs(w.max_rad - w.min_rad - 2 * MAX_WAVE_AMPLITUDE_RAD) < 1e-9);
        }
        const park = p.steps[p.steps.indexOf(waves[0]) - 1];
        assert.ok(park.kind === "hold" && waves[0].kind === "wave" && park.target_rad === waves[0].min_rad, "parked at the wave's start");
      });
      const script = jointCalibrationScript(p, { operator: "bench", settleSec: 2.5, measureSec: 1.5, returnHomeSec: 6 });
      assert.ok(!script.includes("sleep 4"), "parks settle only (settle_sec), no measure dwell");
      assert.ok(scriptSleepTotalSec(script) <= MAX_SESSION_SLEEP_SEC, `${joint} ${scriptSleepTotalSec(script)}`);
      const motion = script.filter((l) => /^(hold-at|wave) /.test(l));
      assert.equal(stepsSeenInTrace(p.steps, traceFor(motion)), p.steps.length);
    }
  });

  it("poses closer than 2a refuse; τ at the poses comes from the guard batch", () => {
    const close = wavePlan(PITCH, 0.05, { posesRad: [0, 0.09, 0.3] });
    const spacing = checkWavePoseSpacing(close);
    assert.ok(!spacing.ok && /0\.09 rad apart, not more than 2 × wave amplitude 0\.05 rad/.test(spacing.message));
    const p = wavePlan(PITCH, 0.05, { posesRad: [-0.4, 0, 0.4] });
    const configs = guardConfigurations(p);
    const tau = new Map(configs.map((c, i) => [i, { [PITCH]: 2 * Math.sin(c[PITCH] ?? 0) }]));
    assert.ok(Math.abs((maxSweepTauAtPoses(p, configs, tau) ?? 0) - 2 * Math.sin(0.4)) < 1e-12);
  });

  it("runs a wave session by default: τ batch, derived amplitude, plan.json method wave", async () => {
    const tauOf = (q: number) => Math.round(2.4 * Math.sin(q) * 1e4) / 1e4;
    const h = harness({ tau: (q) => ({ [PITCH]: tauOf(q[PITCH] ?? 0) }) });
    const out = await h.run({ ...OPT_INS, sweep_joint: PITCH });
    assert.equal(h.bodies.length, 6, "pre-flight, τ batch, gate snapshot, gate preview, session, trace");
    assert.match(h.bodies[1], /@@gravcal_pose/);
    const planJson = JSON.parse(h.writes.get(`${CAL_DIR}/plan.json`) ?? "{}");
    assert.equal(planJson.method, "wave");
    assert.equal(planJson.session_complete, true);
    const poses: number[] = planJson.poses_rad;
    const expectedPoses = defaultPoses(repoLimits()[PITCH].window, 0.25, MAX_WAVE_AMPLITUDE_RAD);
    assert.ok(expectedPoses.ok);
    assert.deepEqual(poses, expectedPoses.poses);
    const maxTau = Math.max(...poses.map((q) => Math.abs(tauOf(q))));
    const expected = deriveWaveAmplitude(PITCH, repoLimits()[PITCH], maxTau);
    assert.ok(expected.ok);
    assert.equal(planJson.wave_amplitude_rad, expected.amplitudeRad);
    assert.ok(!("approach_offset_rad" in planJson) && !("measure_sec" in planJson));
    const waves = planJson.steps.filter((s: { kind: string }) => s.kind === "wave");
    assert.equal(new Set(waves.map((w: { pose_index: number }) => w.pose_index)).size, poses.length);
    assert.match(h.writes.get(`${CAL_DIR}/bench-session.txt`) ?? "", /wave amplitude 0\.\d+ rad = max\(0\.025, \(F_s 0\.08/);
    assert.equal(h.fits.length, 1);
    assert.match(out, /gravity-fit: proposal written/);
    assert.deepEqual(h.audits, [{ tool: "pi_joint_calibrate", exitCode: 0 }]);
  });

  it("velocity_passes belong to the static method", async () => {
    const h = harness();
    const out = await h.run({
      ...OPT_INS,
      sweep_joint: ELBOW,
      velocity_passes: { min_rad: 0.1, max_rad: 0.3, half_periods_s: [1], cycles: 1 },
    });
    assert.match(out, /^Refused: velocity_passes is for method static/);
    assert.equal(h.bodies.length, 1, "pre-flight only");
  });
});

function roundTo3(x: number): number {
  return Math.round(x * 1000) / 1000;
}
