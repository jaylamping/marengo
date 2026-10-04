import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import type { MarengoPiConfig } from "../src/config.js";
import { MASTER_JOINTS } from "../src/bench-profiles.js";
import { type CalibrationDeps, MAX_SESSION_SLEEP_SEC } from "../src/tools/calibration-session.js";
import { GRAVITY_INDEX_PATH, type TauFactors, UNVERIFIED_TAU_FACTOR, resolveTauFactors } from "../src/tau-factor.js";
import {
  type JointLimits,
  admissibleWaveSpeed,
  readJointLimits,
  waveDescentFactor,
  wavePeakSpeed,
} from "../src/tools/joint-calibrate.js";
import { benchLogWrapper } from "../src/tools/motion.js";
import {
  type MotionSuiteArgs,
  type SuiteGeometry,
  type SuiteOptions,
  SUITE_SESSIONS,
  planSuite,
  registerMotionSuiteTools,
  reversalSleep,
  SCORE_TAIL_LINES,
  scoreBlock,
  suiteBand,
  suiteGeometry,
  suiteWaveHalfPeriod,
  tauGridSamples,
} from "../src/tools/motion-suite.js";
import { gravityPreviewReply, isGravityPreviewBody, localFiles } from "./gravity-fixture.js";

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
const [PITCH, , , ELBOW] = MASTER_JOINTS;
const CHAIN = [...MASTER_JOINTS];

/** Pitch-like gravity: 3.2·sin q on the pitch, 0 elsewhere (τ guard band |τ| ≤ 2.5 Nm). */
const PITCH_TAU = (q: Record<string, number>): Record<string, number> => ({ [PITCH]: 3.2 * Math.sin(q[PITCH] ?? 0) });
/** The 2026-10-04 wave fit applied to the URDF: A 2.661 sin q + B 0.038 cos q. */
const FITTED_PITCH_TAU = (q: Record<string, number>): Record<string, number> => {
  const p = q[PITCH] ?? 0;
  return { [PITCH]: 2.661 * Math.sin(p) + 0.038 * Math.cos(p) };
};
const UNVERIFIED: TauFactors = { at: () => UNVERIFIED_TAU_FACTOR, report: [] };
const RECORD_REL = "docs/commissioning/calibrations/2026-10-04-gravity-20261004T113836Z-20261004T113951Z.json";
/** The repo calibrations index and the pitch record, under cfg.localRoot. */
const CALIBRATION_FILES = Object.fromEntries([GRAVITY_INDEX_PATH, RECORD_REL].map((rel) => [`${cfg.localRoot}/${rel}`, repoFile(rel)]));

const OPTIONS: SuiteOptions = {
  spanFractions: [0.25, 0.5, 0.9],
  speedFractions: [0.25, 0.5, 0.9],
  shortMovesRad: [0.02, 0.05, 0.1],
  holdSec: 10,
  repeatCount: 5,
  settleSec: 2.5,
  returnHomeSec: 6,
  operator: "bench",
};

function repoLimits(control = FILES.controlYaml, motors = FILES.motorsYaml): Record<string, JointLimits> {
  const read = readJointLimits({ ...FILES, controlYaml: control, motorsYaml: motors }, CHAIN);
  assert.ok(read.ok, read.ok ? "" : read.message);
  return read.limits;
}

function geometry(
  joint: string,
  tau: (q: Record<string, number>) => Record<string, number>,
  factors = UNVERIFIED,
  limits = repoLimits(),
): SuiteGeometry {
  const samples = tauGridSamples(limits[joint].window);
  const g = suiteGeometry(joint, samples, samples.map((q) => ({ ...Object.fromEntries(CHAIN.map((j) => [j, 0])), ...tau({ [joint]: q }) })), CHAIN, limits, factors);
  assert.ok(g.ok, g.ok ? "" : g.message);
  return g;
}

describe("pi_motion_suite geometry", () => {
  it("τ grid covers the inset window every 0.05 rad, its edges and 0", () => {
    const w = repoLimits()[PITCH].window;
    const s = tauGridSamples(w);
    assert.equal(s[0], Math.round((w.lower + 0.05) * 1e9) / 1e9);
    assert.equal(s.at(-1), Math.round((w.upper - 0.05) * 1e9) / 1e9);
    assert.ok(s.includes(0) && s.includes(0.85) && s.includes(-0.85));
  });

  it("the τ guard bounds the pitch's usable window and sets the gravity extremes", () => {
    const g = geometry(PITCH, PITCH_TAU);
    // 1.6 · 3.2 · |sin q| ≤ 0.8 · 5 Nm ⇔ |q| ≤ 0.895 rad: the last passing grid samples are ±0.85.
    assert.deepEqual([g.lower, g.upper, g.lowerBound, g.upperBound], [-0.85, 0.85, "tau", "tau"]);
    assert.deepEqual([g.gLo, g.gHi], [-0.85, 0.85]);
  });

  it("without gravity the limits bound the window", () => {
    const w = repoLimits()[PITCH].window;
    const g = geometry(PITCH, () => ({}));
    assert.equal(g.lowerBound, "limit");
    assert.equal(g.upperBound, "limit");
    // The inset window edges, rounded inward to the mrad.
    const lo = w.lower + 0.05;
    const hi = w.upper - 0.05;
    assert.ok(g.lower >= lo && g.lower - lo < 1e-3 && g.upper <= hi && hi - g.upper < 1e-3, `${g.lower} ${g.upper}`);
  });

  it("a calibrated pitch (× 1.15) reaches the soft ∩ hard limits at both ends; unverified (× 1.6) stops at 1.2", async () => {
    const calibrated = await resolveTauFactors({ urdf: FILES.urdf, joints: CHAIN, readLocal: async (rel) => repoFile(rel) });
    const g = geometry(PITCH, FITTED_PITCH_TAU, calibrated);
    // max(soft −1.0882, hard −1.1152) + 0.05 and min(soft 2.9801, hard 3.0071) − 0.05, inward to the mrad;
    // 1.15 × 2.661 Nm = 3.06 Nm ≤ 0.8 × 5 Nm everywhere.
    assert.deepEqual([g.lower, g.upper, g.lowerBound, g.upperBound], [-1.038, 2.93, "limit", "limit"]);
    assert.deepEqual([g.gLo, g.gHi], [-1.038, 1.55]);
    const unverified = geometry(PITCH, FITTED_PITCH_TAU);
    // 1.6 × τ_g(1.25) = 4.06 Nm > 4 Nm.
    assert.deepEqual([unverified.lower, unverified.upper, unverified.upperBound], [-1.038, 1.2, "tau"]);
  });

  it("the window is soft ∩ hard at both ends, whichever is tighter", () => {
    const motors = FILES.motorsYaml.replace("position_lower_rad: -1.1152385473251345", "position_lower_rad: -1.0").replace(
      "position_upper_rad: 3.007078170776367",
      "position_upper_rad: 2.9",
    );
    assert.notEqual(motors, FILES.motorsYaml);
    const hardTighter = geometry(PITCH, () => ({}), UNVERIFIED, repoLimits(FILES.controlYaml, motors));
    assert.deepEqual([hardTighter.lower, hardTighter.upper], [-0.95, 2.85]);
    const softTighter = geometry(PITCH, () => ({}));
    assert.deepEqual([softTighter.lower, softTighter.upper], [-1.038, 2.93]);
  });

  it("drops a gravity extreme within 0.15 rad of 0 (the elbow's negative side)", () => {
    const g = geometry(ELBOW, (q) => ({ [ELBOW]: 0.5 * Math.sin(q[ELBOW] ?? 0) }));
    assert.equal(g.gLo, undefined);
    assert.ok(g.gHi !== undefined && g.gHi > 0.15);
  });

  it("refuses a usable window narrower than 0.2 rad, and a missing τ", () => {
    const limits = repoLimits();
    const samples = tauGridSamples(limits[PITCH].window);
    const steep = samples.map((q) => ({ ...Object.fromEntries(CHAIN.map((j) => [j, 0])), [PITCH]: 40 * q }));
    const narrow = suiteGeometry(PITCH, samples, steep, CHAIN, limits, UNVERIFIED);
    assert.ok(!narrow.ok && /narrower than 0.2 rad/.test(narrow.message));
    const missing = suiteGeometry(PITCH, samples, samples.map(() => undefined), CHAIN, limits, UNVERIFIED);
    assert.ok(!missing.ok && /τ guard unavailable/.test(missing.message));
  });
});

describe("pi_motion_suite plan", () => {
  it("single-cycle wave half periods hit the speed fraction of the admissible speed", () => {
    const l = repoLimits()[PITCH];
    for (const span of [0.425, 0.85, 1.53, 1.7]) {
      const vAdm = admissibleWaveSpeed(l, -span / 2, span / 2);
      for (const share of [0.25, 0.5, 0.9]) {
        const half = suiteWaveHalfPeriod(l, -span / 2, span / 2, share);
        const peak = wavePeakSpeed(0, span, half);
        assert.ok(peak <= share * vAdm + 1e-9 && peak >= 0.97 * share * vAdm, `${span} ${share}: ${peak} vs ${share * vAdm}`);
      }
    }
  });

  it("keeps every wave's descent above the pitch danger zone within its speed", () => {
    // Master pitch: clamp_velocity above position_above_rad. Davout would clamp a faster
    // descent there and, with drive damping, brake the wave (bench 2026-10-04: +29–37 %).
    const l = repoLimits()[PITCH];
    const cap = l.descentCap;
    assert.ok(cap !== undefined, "master pitch has a descent zone");
    const [lo, hi] = [-1.038, 1.2];
    const factor = waveDescentFactor(lo, hi, cap.aboveRad);
    assert.ok(factor > 0 && factor < 1);
    const vAdm = admissibleWaveSpeed(l, lo, hi);
    assert.ok(Math.abs(vAdm - cap.maxVelocityRadS / factor) < 1e-12, `${vAdm}`);
    const half = suiteWaveHalfPeriod(l, lo, hi, 0.9);
    assert.ok(wavePeakSpeed(lo, hi, half) * factor <= cap.maxVelocityRadS + 1e-9);
    // A band that never reaches the threshold keeps the other limits only.
    assert.equal(waveDescentFactor(-0.5, cap.aboveRad, cap.aboveRad), 0);
    const g = geometry(PITCH, PITCH_TAU);
    const p = planSuite({ geometry: g, chain: CHAIN, limits: repoLimits(), sessions: SUITE_SESSIONS, options: OPTIONS });
    assert.ok(p.ok, p.ok ? "" : p.message);
    for (const part of p.parts) {
      for (const s of part.steps) {
        if (s.kind !== "wave") continue;
        const descent = wavePeakSpeed(s.min_rad, s.max_rad, s.half_period_s) * waveDescentFactor(s.min_rad, s.max_rad, cap.aboveRad);
        assert.ok(descent <= cap.maxVelocityRadS + 1e-9, `${part.name} [${s.min_rad}, ${s.max_rad}] T ${s.half_period_s}: ${descent}`);
      }
    }
  });

  it("reversal sleep is the trapezoid time to half the move, to 0.05 s", () => {
    // Half of 1 rad at 1.25 rad/s, 1.5 rad/s²: inside the accel ramp, √(2·0.5/1.5) = 0.816 s.
    assert.equal(reversalSleep(0.5, 1.25, 1.5), 0.8);
    // 1.5 rad: ramp 0.521 rad, then cruise: (1.5 − 0.521)/1.25 + 1.25/1.5 = 1.617 s.
    assert.equal(reversalSleep(1.5, 1.25, 1.5), 1.6);
  });

  it("plans every session within the 300 s budget from the master pitch config", () => {
    const g = geometry(PITCH, PITCH_TAU);
    const p = planSuite({ geometry: g, chain: CHAIN, limits: repoLimits(), sessions: SUITE_SESSIONS, options: OPTIONS });
    assert.ok(p.ok, p.ok ? "" : p.message);
    assert.deepEqual(p.parts.map((x) => x.name), [...SUITE_SESSIONS]);
    for (const part of p.parts) assert.ok(part.budgetSec <= MAX_SESSION_SLEEP_SEC, `${part.name} ${part.budgetSec}`);
    const long = p.parts[0];
    assert.equal(long.steps.filter((s) => s.kind === "wave").length, 9);
    // Bands centred on 0: 25/50/90 % of the 1.7 rad window.
    assert.deepEqual(suiteBand(g, 0.25), [-0.212, 0.212]);
    const extremes = p.parts.find((x) => x.name === "gravity_extremes");
    assert.deepEqual(
      extremes?.steps.map((s) => (s.kind === "hold" ? s.target_rad : NaN)),
      [-0.85, 0.85, -0.85],
    );
    const sleeps = extremes?.script.filter((l) => l.startsWith("sleep ")).map((l) => Number(l.slice(6)));
    assert.ok(sleeps !== undefined && sleeps[0] >= 10 && sleeps[1] >= 10, `${sleeps}`);
    const repeat = p.parts.find((x) => x.name === "repeatability");
    assert.equal(repeat?.steps.length, 10);
    const reversal = p.parts.find((x) => x.name === "sweeps_and_reversals")?.script ?? [];
    // hold-at a, settle; hold-at b; √(2 · 0.212 / 1.5) = 0.53 s → 0.55; hold-at a again.
    const i = reversal.indexOf("hold-at right_shoulder_pitch 0.212");
    assert.ok(i > 0 && reversal[i + 1] === "sleep 0.55" && reversal[i + 2] === "hold-at right_shoulder_pitch -0.212", reversal.join("\n"));
  });

  it("splits a session that exceeds 300 s at block boundaries", () => {
    const g = geometry(PITCH, PITCH_TAU);
    const p = planSuite({
      geometry: g,
      chain: CHAIN,
      limits: repoLimits(),
      sessions: ["repeatability"],
      options: { ...OPTIONS, repeatCount: 10, settleSec: 12 },
    });
    assert.ok(p.ok, p.ok ? "" : p.message);
    assert.deepEqual(p.parts.map((x) => x.name), ["repeatability_1", "repeatability_2"]);
    for (const part of p.parts) {
      assert.ok(part.budgetSec <= MAX_SESSION_SLEEP_SEC);
      // Each part starts from rest with its own move out.
      assert.equal(part.steps[0].kind === "hold" && part.steps[0].target_rad, suiteBand(g, 0.25)[1]);
    }
    assert.equal(p.parts.reduce((n, x) => n + x.steps.length, 0), 20);
  });

  it("refuses without the trajectory accel to time moves", () => {
    const control = FILES.controlYaml.replace(/ +position_trajectory_accel_rad_s2: 1\.5\n/, "");
    const limits = repoLimits(control);
    const g = geometry(PITCH, PITCH_TAU);
    const p = planSuite({ geometry: g, chain: CHAIN, limits, sessions: ["long_moves"], options: OPTIONS });
    assert.ok(!p.ok && /position_trajectory_accel_rad_s2/.test(p.message));
  });
});

// ---------------------------------------------------------------------------
// Tool harness

function marked(name: string, content: string): string {
  return `=====MARENGO_GRAVCAL_${name}_BEGIN=====\n${content}\n=====MARENGO_GRAVCAL_${name}_END=====`;
}

const PREFLIGHT = [
  marked("robot", FILES.robotYaml),
  marked("control", FILES.controlYaml),
  marked("motors", FILES.motorsYaml),
  "MARENGO_GRAVCAL_URDF_PATH=/opt/marengo/assets/urdf/marengo.urdf",
  marked("urdf", FILES.urdf),
].join("\n");

const TRACE_HEADER =
  "tick,t_ms,joint,q,dq,q_traj,dq_traj,q_des,target,target_raw,q_env_lo,q_env_hi,lead,lead_sat,settle_error,phase,friction_mode,tau_p,tau_g,tau_f,tau_d,tau_ff_cmd,tau_meas,dq_mit,kp,kd,joint_stuck,planner_frozen,retarget_age_ms,planner_event,law,q_ref,dq_ref,time_scale,tau_i,kd_mit,tau_ff_wire";

/** Pitch trace rows the hold-at / wave stdin lines of a session would leave (50 Hz). */
function traceFor(lines: readonly string[]): string {
  const rows = [TRACE_HEADER];
  let tick = 0;
  let target = 0;
  const emit = () => {
    const t = target.toFixed(6);
    rows.push(`${tick},${tick * 20},${PITCH},${t},0,${t},0,${t},${t},${t},-1,3,0,0,0,Hold,static,0,0,0,0,0,0,0,18,3,0,0,0,tick,scaled_pd,${t},0,1,0,3,0`);
    tick += 1;
  };
  emit();
  for (const line of lines) {
    const [verb, joint, ...nums] = line.split(" ");
    if (joint !== PITCH) continue;
    if (verb === "hold-at") {
      target = Number(nums[0]);
      for (let k = 0; k < 10; k += 1) emit();
    } else if (verb === "wave") {
      const [min, max, cycles, half] = nums.map(Number);
      const n = Math.round(cycles * 2 * half * 50);
      for (let k = 0; k <= n; k += 1) {
        target = min + ((max - min) * (1 - Math.cos((Math.PI * k) / (half * 50)))) / 2;
        emit();
      }
    }
  }
  return `${rows.join("\n")}\n`;
}

const stdinLines = (body: string) => [...body.matchAll(/printf '%s\\n' "([^"]+)"/g)].map((m) => m[1]);

interface Harness {
  bodies: string[];
  writes: Map<string, string>;
  scorer: string[][];
  audits: { tool: string; exitCode: number }[];
  run: (args: Partial<MotionSuiteArgs>) => Promise<string>;
}

function harness(
  opts: {
    refuseSession?: string;
    scorerExit?: number;
    scorerStdout?: string;
    files?: Record<string, string>;
    tau?: typeof PITCH_TAU;
  } = {},
): Harness {
  const h: Harness = { bodies: [], writes: new Map(), scorer: [], audits: [], run: async () => "" };
  let session = "";
  let n = 0;
  const runRemote = async (body: string) => {
    h.bodies.push(body);
    if (body.includes("MARENGO_GRAVCAL_URDF_PATH=")) return PREFLIGHT;
    if (body.includes("@@gravcal_pose")) {
      const rows = /printf '%s\\n' ((?:'[^']*' ?)+) \| while/.exec(body)?.[1] ?? "";
      return [...rows.matchAll(/'([^']*)'/g)]
        .map((m) => {
          const [index, ...values] = m[1].split(" ").map(Number);
          const q = Object.fromEntries(CHAIN.map((j, i) => [j, values[i]]));
          return `@@gravcal_pose ${index}\n${gravityPreviewReply((opts.tau ?? PITCH_TAU)(q))}`;
        })
        .join("\n");
    }
    if (isGravityPreviewBody(body)) return gravityPreviewReply();
    if (body.includes("pi_now_ms=")) return "";
    const label = /LABEL='(motion-suite-[a-z_0-9]+)'/.exec(body)?.[1];
    if (label !== undefined) {
      session = body;
      n += 1;
      const ts = `20261004T13000${n}Z`;
      if (label === `motion-suite-${opts.refuseSession}`) return "enable refused: gravity preflight\n[exit 1]";
      return `=== bench session ${ts} ===\n{"log":"/opt/marengo/var/log/bench-${ts}.log","trace":"/opt/marengo/var/log/position-trace-${ts}.csv","candump":"","ts":"${ts}","label":"${label}"}`;
    }
    if (body.includes("MARENGO_GRAVCAL_trace_BEGIN")) return marked("trace", traceFor(stdinLines(session)));
    throw new Error(`unexpected remote body:\n${body.slice(0, 400)}`);
  };
  const deps: CalibrationDeps = {
    execLocal: async (_command, args) => {
      h.scorer.push(args);
      const exitCode = opts.scorerExit ?? 0;
      return {
        stdout:
          opts.scorerStdout ??
          `trace: x\n\n=== ADR 0039 bench score: ${PITCH} (…) ===\n  move 1 [move]: PASS\n  verdict: ${exitCode === 0 ? "PASS" : "FAIL"} (1/1 moves pass: move 1/1)\n`,
        stderr: "",
        exitCode,
      };
    },
    writeFile: async (file, data) => {
      h.writes.set(file, data);
    },
    readFile: localFiles(opts.files),
    mkdir: async () => {},
    now: () => new Date("2026-10-04T13:00:00.000Z"),
  };
  const tools = registerMotionSuiteTools(cfg, runRemote, (tool, _a, _o, exitCode) => h.audits.push({ tool, exitCode }), deps);
  h.run = (args) =>
    tools.pi_motion_suite.handler({
      confirm: true,
      confirm_weighted_motion: true,
      set_zero: true,
      at_mechanical_reference: true,
      ...args,
    } as MotionSuiteArgs);
  return h;
}

const sessionBodies = (h: Harness) => h.bodies.filter((b) => /LABEL='motion-suite-/.test(b));

describe("pi_motion_suite tool", () => {
  it("refuses without set_zero + at_mechanical_reference, before any remote call", async () => {
    const h = harness();
    const out = await h.run({ set_zero: false });
    assert.match(out, /set_zero: true/);
    assert.equal(h.bodies.length, 0);
  });

  it("dry_run plans and guards every session without running marengo-pi", async () => {
    const h = harness();
    const out = await h.run({ dry_run: true });
    assert.match(out, /usable window \[-0\.85, 0\.85\] rad \(lower bound: tau, upper: tau\)/);
    for (const name of SUITE_SESSIONS) assert.match(out, new RegExp(`  ${name}: budget \\d+(\\.\\d+)? s`));
    assert.match(out, /dry_run: no session was run/);
    assert.equal(sessionBodies(h).length, 0);
  });

  it("dry_run needs only confirm and never takes CAN ownership", async () => {
    const h = harness();
    const out = await h.run({
      dry_run: true,
      confirm_weighted_motion: undefined,
      set_zero: undefined,
      at_mechanical_reference: undefined,
    });
    assert.match(out, /dry_run: no session was run/);
    assert.ok(h.bodies.length > 0, "the dry run still reads config and runs the τ batches");
    for (const b of h.bodies) assert.doesNotMatch(b, /marengo-pi\.service restore after session/);
    assert.equal(sessionBodies(h).length, 0);
  });

  it("dry_run still refuses without confirm", async () => {
    const h = harness();
    const out = await h.run({ dry_run: true, confirm: undefined } as unknown as Partial<MotionSuiteArgs>);
    assert.match(out, /confirm: true/);
    assert.equal(h.bodies.length, 0);
  });

  it("a real run refuses a weighted profile without confirm_weighted_motion, before any remote call", async () => {
    const h = harness();
    const out = await h.run({ confirm_weighted_motion: undefined });
    assert.match(out, /Weighted motion blocked/);
    assert.equal(h.bodies.length, 0);
  });

  it("a real run takes CAN ownership for its τ batches", async () => {
    const h = harness();
    await h.run({ sessions: ["repeatability"] });
    const tauBodies = h.bodies.filter((b) => b.includes("@@gravcal_pose"));
    assert.ok(tauBodies.length > 0);
    for (const b of tauBodies) assert.match(b, /marengo-pi\.service restore after session/);
  });

  it("dry_run with the repo calibrations index widens the pitch window to the soft ∩ hard limits", async () => {
    const h = harness({ files: CALIBRATION_FILES, tau: FITTED_PITCH_TAU });
    const out = await h.run({ dry_run: true });
    assert.match(out, /usable window \[-1\.038, 2\.93\] rad \(lower bound: limit, upper: limit\)/);
    assert.match(out, /right_shoulder_pitch: × 1\.15 calibrated/);
    assert.match(out, /right_shoulder_pitch: max \|τ_g\| 2\.66\d Nm at right_shoulder_pitch=1\.5\d*; × 1\.15 = 3\.06\d Nm vs 0\.8 × cap 5 Nm = 4\.000 Nm ok/);
    for (const name of SUITE_SESSIONS) assert.match(out, new RegExp(`  ${name}(_\\d+)?: budget \\d+(\\.\\d+)? s`));
    assert.equal(sessionBodies(h).length, 0);
  });

  it("runs every session with the sweep joint traced every tick, scores each and summarizes", async () => {
    const h = harness();
    const out = await h.run({ sessions: ["gravity_extremes", "repeatability"] });
    const bodies = sessionBodies(h);
    assert.equal(bodies.length, 2);
    for (const b of bodies) assert.match(b, /export MARENGO_POSITION_TRACE_FULL_RATE_JOINTS='right_shoulder_pitch'/);
    assert.equal(h.scorer.length, 2);
    assert.deepEqual(h.scorer[0].slice(0, 1), ["scripts/analyze-position-trace.py"]);
    assert.ok(h.scorer[0].includes("--score-bench") && h.scorer[0].includes(PITCH));
    assert.ok(h.writes.has("/tmp/marengo/var/motion-suite/20261004T130001Z/score.txt"));
    const plan = JSON.parse(h.writes.get("/tmp/marengo/var/motion-suite/20261004T130001Z/plan.json") ?? "{}");
    assert.equal(plan.session, "gravity_extremes");
    assert.equal(plan.session_complete, true);
    assert.match(out, /gravity_extremes \| 20261004T130001Z \| \d+(\.\d+)? \| PASS/);
    assert.match(out, /repeatability \| 20261004T130002Z \| \d+(\.\d+)? \| PASS/);
    assert.match(out, /overall: PASS/);
    assert.deepEqual(h.audits.at(-1), { tool: "pi_motion_suite", exitCode: 0 });
  });

  it("survives a huge scorer output without a bench-score block and keeps the result bounded", async () => {
    // 2026-10-04: 51,369 legacy segments filled the 16 MiB exec buffer, the score block was cut
    // off and spreading every line into one push overflowed the stack.
    const huge = Array.from({ length: 400_000 }, (_, i) => `--- segment ${i} ---`).join("\n");
    const h = harness({ scorerExit: 2, scorerStdout: huge });
    const out = await h.run({ sessions: ["repeatability"] });
    assert.match(out, /no bench-score block; last 40 of 400000 scorer lines/);
    assert.ok(out.split("\n").length < 200, "tool output stays bounded");
  });

  it("scoreBlock keeps everything from the bench-score header, or a bounded tail", () => {
    assert.deepEqual(scoreBlock("a\n=== ADR 0039 bench score: x ===\n  verdict: PASS\n"), [
      "=== ADR 0039 bench score: x ===",
      "  verdict: PASS",
    ]);
    const tail = scoreBlock(Array.from({ length: 100 }, (_, i) => `l${i}`).join("\n"));
    assert.equal(tail.length, SCORE_TAIL_LINES + 1);
    assert.equal(tail.at(-1), "l99");
  });

  it("stops at a refused session and fails the suite", async () => {
    const h = harness({ refuseSession: "short_moves" });
    const out = await h.run({ sessions: ["short_moves", "repeatability"] });
    assert.equal(sessionBodies(h).length, 1);
    assert.equal(h.scorer.length, 0);
    assert.match(out, /short_moves \| - \| \d+(\.\d+)? \| not run \(refused or failed\)/);
    assert.match(out, /repeatability \| - \| \d+(\.\d+)? \| not run \(suite stopped\)/);
    assert.match(out, /overall: FAIL/);
  });

  it("a scorer FAIL fails the suite but the next session still runs", async () => {
    const h = harness({ scorerExit: 2 });
    const out = await h.run({ sessions: ["gravity_extremes", "repeatability"] });
    assert.equal(sessionBodies(h).length, 2);
    assert.match(out, /gravity_extremes \| 20261004T130001Z \| \d+(\.\d+)? \| FAIL/);
    assert.match(out, /overall: FAIL/);
  });
});

describe("benchLogWrapper full-rate trace", () => {
  it("exports MARENGO_POSITION_TRACE_FULL_RATE_JOINTS only when joints are given", () => {
    assert.match(benchLogWrapper(cfg, "true", "x", undefined, [PITCH, ELBOW]), /export MARENGO_POSITION_TRACE_FULL_RATE_JOINTS='right_shoulder_pitch,right_elbow_pitch'/);
    assert.doesNotMatch(benchLogWrapper(cfg, "true", "x"), /FULL_RATE/);
    assert.throws(() => benchLogWrapper(cfg, "true", "x", undefined, ["a;b"]));
  });
});
