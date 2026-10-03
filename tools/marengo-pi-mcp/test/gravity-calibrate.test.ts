import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import type { MarengoPiConfig } from "../src/config.js";
import { MASTER_JOINTS } from "../src/bench-profiles.js";
import {
  type GravityCalibrateArgs,
  type GravityCalibrateDeps,
  checkPlanLimits,
  extractMarked,
  jointWindows,
  parsePreflight,
  parseSessionJson,
  parseYamlLite,
  planCalibrationSweep,
  preflightReadShell,
  registerGravityCalibrateTools,
} from "../src/tools/gravity-calibrate.js";
import { benchLogWrapper } from "../src/tools/motion.js";
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

const REPO_CONFIG = new URL("../../../config/", import.meta.url);
const TS = "20261003T120000Z";
const TRACE_PATH = `/opt/marengo/var/log/position-trace-${TS}.csv`;
const TRACE_CSV = "tick,t_ms,joint,q,dq,target_raw,tau_meas\n0,0,right_shoulder_pitch,0,0,0,0.1\n";
const URDF = '<robot name="marengo"><link name="right_upper_arm"/></robot>\n';

const ROBOT_YAML = [
  "# master",
  "robot:",
  "  name: marengo_arm_5dof_right",
  "  urdf: assets/urdf/marengo.urdf",
  "  joints:",
  "    - right_shoulder_pitch",
  "    - right_elbow_pitch",
  "",
].join("\n");

type Bounds = [number | undefined, number | undefined];

function controlYaml(pitch: Bounds, elbow: Bounds = [-0.38, 1.0]): string {
  const soft = ([lo, hi]: Bounds) => [
    ...(lo === undefined ? [] : [`      position_soft_lower_rad: ${lo}`]),
    ...(hi === undefined ? [] : [`      position_soft_upper_rad: ${hi}`]),
  ];
  return [
    "control:",
    "  loop_hz: 200",
    "  actuator_groups:",
    "    lower_arm:",
    "      joints: [right_lower_arm_yaw]",
    "  joints:",
    "    right_shoulder_pitch:",
    "      motor_type: rs03",
    "      impedance:",
    "        kp: 18.0 # stiff",
    "      # comment between keys",
    ...soft(pitch),
    "      friction:",
    "        fc: 0.08",
    "    right_elbow_pitch:",
    ...soft(elbow),
    "",
  ].join("\n");
}

function motorsYaml(pitch: [number, number], elbow: [number, number] = [-0.4, 1.05]): string {
  const motor = (joint: string, [lo, hi]: [number, number]) => [
    `  - joint: ${joint}`,
    "    driver: robstride",
    '    firmware_version: "0.3.1.42"',
    "    bench:",
    `      position_lower_rad: ${lo}`,
    `      position_upper_rad: ${hi}`,
    "      torque_limit_nm: 5.0",
  ];
  return ["# bench", "motors:", ...motor("right_shoulder_pitch", pitch), ...motor("right_elbow_pitch", elbow), ""].join(
    "\n",
  );
}

function marked(name: string, content: string): string {
  return `=====MARENGO_GRAVCAL_${name}_BEGIN=====\n${content}\n=====MARENGO_GRAVCAL_${name}_END=====`;
}

function preflightReply(control = controlYaml([-0.87, 2.9]), motors = motorsYaml([-0.9, 2.92])): string {
  return [
    marked("robot", ROBOT_YAML),
    marked("control", control),
    marked("motors", motors),
    "MARENGO_GRAVCAL_URDF_PATH=/opt/marengo/assets/urdf/marengo.urdf",
    marked("urdf", URDF),
  ].join("\n");
}

const SESSION_JSON = `{"log":"/opt/marengo/var/log/bench-${TS}.log","trace":"${TRACE_PATH}","candump":"","ts":"${TS}","label":"gravity-calibrate"}`;

interface Harness {
  bodies: string[];
  timeouts: (number | undefined)[];
  writes: Map<string, string>;
  dirs: string[];
  fits: { command: string; args: string[]; opts: { cwd?: string; timeoutMs?: number } }[];
  audits: { tool: string; exitCode: number }[];
  run: (args: Partial<GravityCalibrateArgs>) => Promise<string>;
}

function harness(
  opts: {
    preflight?: string;
    preview?: string;
    session?: string;
    trace?: string;
    fitExit?: number;
    fitStderr?: string;
  } = {},
): Harness {
  const h: Harness = {
    bodies: [],
    timeouts: [],
    writes: new Map(),
    dirs: [],
    fits: [],
    audits: [],
    run: async () => "",
  };
  const runRemote = async (body: string, timeoutMs?: number) => {
    h.bodies.push(body);
    h.timeouts.push(timeoutMs);
    if (body.includes("MARENGO_GRAVCAL_URDF_PATH=")) return opts.preflight ?? preflightReply();
    if (isGravityPreviewBody(body)) return opts.preview ?? gravityPreviewReply();
    if (body.includes("pi_now_ms=")) return "";
    if (body.includes("LABEL='gravity-calibrate'")) {
      return opts.session ?? `=== bench session ${TS} (gravity-calibrate) ===\nstatus ok\n${SESSION_JSON}`;
    }
    if (body.includes("MARENGO_GRAVCAL_trace_BEGIN")) {
      return opts.trace ?? marked("trace", TRACE_CSV);
    }
    throw new Error(`unexpected remote body:\n${body.slice(0, 400)}`);
  };
  const deps: GravityCalibrateDeps = {
    execLocal: async (command, args, o) => {
      h.fits.push({ command, args, opts: o });
      return { stdout: "proposal: docs/commissioning/calibrations/x.patch", stderr: opts.fitStderr ?? "", exitCode: opts.fitExit ?? 0 };
    },
    writeFile: async (file, data) => {
      h.writes.set(file, data);
    },
    mkdir: async (dir) => {
      h.dirs.push(dir);
    },
    now: () => new Date("2026-10-03T12:00:00.000Z"),
  };
  const tools = registerGravityCalibrateTools(
    cfg,
    runRemote,
    (tool, _args, _out, exitCode) => h.audits.push({ tool, exitCode }),
    deps,
  );
  h.run = (args) => tools.pi_gravity_calibrate.handler({ confirm: true, ...args } as GravityCalibrateArgs);
  return h;
}

const OPT_INS = { set_zero: true, at_mechanical_reference: true, skip_hanging_rest_gravity_check: true };
const CAL_DIR = `/tmp/marengo/var/gravity-calibration/${TS}`;

function holdAts(body: string): string[] {
  return [...body.matchAll(/printf '%s\\n' "(hold-at \S+ \S+)"/g)].map((m) => m[1]);
}

function stdinLines(body: string): string[] {
  return [...body.matchAll(/printf '%s\\n' "([^"]+)"/g)].map((m) => m[1]);
}

describe("pi_gravity_calibrate refusals before touching the Pi", () => {
  const cases: [string, Partial<GravityCalibrateArgs>, RegExp][] = [
    ["no confirm", { ...OPT_INS, confirm: false as unknown as true }, /^Motion blocked/],
    ["weighted without confirm_weighted_motion", { ...OPT_INS, profile: "arm_attached" }, /^Weighted motion blocked/],
    ["no set_zero", { ...OPT_INS, set_zero: false }, /^Refused: enabling needs a current reference/],
    ["no at_mechanical_reference", { ...OPT_INS, at_mechanical_reference: false }, /^Refused: enabling needs a current reference/],
    ["skip flag false", { ...OPT_INS, skip_hanging_rest_gravity_check: false }, /skip_hanging_rest_gravity_check: true.*hanging-rest \|τ_g\| check is skipped only for this tool/s],
    ["non-zero fixed pitch on a pitch sweep", { ...OPT_INS, fixed_pitch_rad: 0.3 }, /fixed_pitch_rad \(0\.3\) applies only to a right_elbow_pitch sweep/],
    ["duplicate poses", { ...OPT_INS, poses_rad: [0, 0.5, 0.5, 1] }, /must be distinct/],
    ["too few pitch poses", { ...OPT_INS, poses_rad: [0, 0.5] }, /at least 3 distinct poses/],
    ["one pose", { ...OPT_INS, sweep_joint: "right_elbow_pitch", poses_rad: [0.2] }, /needs 2–12 entries/],
    ["non-finite pose", { ...OPT_INS, poses_rad: [0, Number.NaN, 1] }, /finite/],
    [
      "budget overflow",
      { ...OPT_INS, poses_rad: [0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7], settle_sec: 10, measure_sec: 5 },
      /session budget \d+ s .* exceeds 300 s/,
    ],
  ];
  for (const [name, args, pattern] of cases) {
    it(name, async () => {
      const h = harness();
      const out = await h.run(args);
      assert.match(out, pattern);
      assert.equal(h.bodies.length, 0);
      assert.equal(h.writes.size, 0);
    });
  }

  it("refuses a sweep the profile does not reference", async () => {
    const h = harness();
    const out = await h.run({ ...OPT_INS, confirm_weighted_motion: true, profile: "roll_attached", sweep_joint: "right_elbow_pitch" });
    assert.match(out, /roll_attached does not reference right_elbow_pitch/);
    assert.equal(h.bodies.length, 0);
  });
});

describe("pi_gravity_calibrate pose limits (pre-flight, before motion)", () => {
  const cases: [string, string, string, RegExp][] = [
    [
      "soft upper below a pose",
      controlYaml([-0.87, 1.0]),
      motorsYaml([-0.9, 2.92]),
      /right_shoulder_pitch step 5 target 1\.2 rad is outside its allowed window \[-0\.87, 1\] rad \(control\.yaml soft \[-0\.87, 1\] ∩ motors\.yaml hard \[-0\.9, 2\.92\]\)/,
    ],
    [
      "hard upper below a pose",
      controlYaml([-0.87, 2.9]),
      motorsYaml([-0.9, 1.1]),
      /right_shoulder_pitch step 5 target 1\.2 rad is outside its allowed window \[-0\.87, 1\.1\]/,
    ],
    [
      "overshoot pose+δ beyond the window",
      controlYaml([-0.87, 1.22]),
      motorsYaml([-0.9, 2.92]),
      /right_shoulder_pitch step 6 target 1\.25 rad is outside/,
    ],
    [
      "overshoot pose−δ below the window",
      controlYaml([-0.023, 2.9]),
      motorsYaml([-0.9, 2.92]),
      /right_shoulder_pitch step 0 target -0\.05 rad is outside its allowed window \[-0\.023, 2\.9\]/,
    ],
    [
      "missing soft bound",
      controlYaml([-0.87, undefined]),
      motorsYaml([-0.9, 2.92]),
      /right_shoulder_pitch pose limits missing or unparseable on the Pi: control\.yaml control\.joints\.right_shoulder_pitch\.position_soft_upper_rad/,
    ],
  ];
  for (const [name, control, motors, pattern] of cases) {
    it(`refuses on ${name}`, async () => {
      const h = harness({ preflight: preflightReply(control, motors) });
      const out = await h.run(OPT_INS);
      assert.match(out, pattern);
      assert.match(out, /no motion was run/);
      assert.equal(h.bodies.length, 1, "only the read-only pre-flight ran");
      assert.match(h.bodies[0], /MARENGO_GRAVCAL_URDF_PATH=/);
      assert.doesNotMatch(h.bodies[0], /motor-repl|marengo-pi/);
      assert.deepEqual(h.audits, [{ tool: "pi_gravity_calibrate", exitCode: 1 }]);
    });
  }

  it("refuses an elbow sweep whose fixed pitch lies outside the pitch window", async () => {
    const h = harness({ preflight: preflightReply(controlYaml([-0.87, 0.5])) });
    const out = await h.run({ ...OPT_INS, sweep_joint: "right_elbow_pitch", fixed_pitch_rad: 0.6 });
    assert.match(out, /right_shoulder_pitch step 0 target 0\.6 rad is outside/);
    assert.equal(h.bodies.length, 1);
  });

  it("refuses an incomplete pre-flight read", async () => {
    const h = harness({ preflight: marked("robot", ROBOT_YAML) });
    const out = await h.run(OPT_INS);
    assert.match(out, /pre-flight read of the Pi config\/URDF was incomplete/);
    assert.equal(h.bodies.length, 1);
  });

  it("reads the repo master config windows", () => {
    const control = readFileSync(new URL("control.yaml", REPO_CONFIG), "utf8");
    const motors = readFileSync(new URL("motors.yaml", REPO_CONFIG), "utf8");
    const windows = jointWindows(control, motors, [...MASTER_JOINTS]);
    assert.ok(windows.ok, windows.ok ? "" : windows.message);
    for (const joint of MASTER_JOINTS) {
      const w = windows.windows[joint];
      assert.ok(w.lower < 0 && w.upper > 0, `${joint} window brackets 0`);
      assert.equal(w.lower, Math.max(w.soft[0], w.hard[0]));
      assert.equal(w.upper, Math.min(w.soft[1], w.hard[1]));
    }
    for (const sweepJoint of ["right_shoulder_pitch", "right_elbow_pitch"] as const) {
      const plan = planCalibrationSweep({
        sweepJoint,
        posesRad: sweepJoint === "right_shoulder_pitch" ? [0, 0.25, 0.48, 0.8, 1.2] : [0, 0.25, 0.5, 0.75],
        fixedPitchRad: 0,
        approachOffsetRad: 0.05,
      });
      assert.ok(plan.ok);
      assert.deepEqual(checkPlanLimits(plan, windows.windows), { ok: true });
    }
  });
});

describe("pi_gravity_calibrate session", () => {
  it("runs one marengo-pi session whose hold-at lines are the plan steps, then writes the contract files and fits", async () => {
    const h = harness();
    const out = await h.run({ ...OPT_INS, fit_params: ["mass:right_upper_arm", "com:right_upper_arm"] });

    assert.equal(h.bodies.length, 5, "preflight, gate snapshot, gate preview, session, trace fetch");
    assert.match(h.bodies[0], /MARENGO_GRAVCAL_URDF_PATH=/);
    assert.match(h.bodies[1], /pi_now_ms=/);
    assert.ok(isGravityPreviewBody(h.bodies[2]));
    const sessions = h.bodies.filter((b) => b.includes("LABEL='gravity-calibrate'"));
    assert.equal(sessions.length, 1);
    const session = h.bodies[3];
    assert.equal(session, sessions[0]);
    assert.match(h.bodies[4], /MARENGO_GRAVCAL_trace_BEGIN/);
    assert.match(h.bodies[4], new RegExp(`cat '${TRACE_PATH}'`));
    assert.doesNotMatch(h.bodies[4], /motor-repl|marengo-pi /);

    const lines = stdinLines(session);
    assert.equal(lines.filter((l) => /^home .* sign-tested$/.test(l)).length, 1);
    assert.equal(lines[0], `home ${MASTER_JOINTS.join(" ")} sign-tested`);
    assert.equal(lines[1], "home");
    assert.equal(lines.filter((l) => l.startsWith("enable")).length, 1);
    assert.equal(lines[2], "enable bench");
    assert.deepEqual(lines.slice(-4), ["hold-at right_shoulder_pitch 0", "status", "disable", "quit"]);
    assert.match(session, /sleep 4;\n/);
    // 50 s reference + 12 steps × 4 s + 6 s return = 104 s.
    assert.match(session, /\} \| timeout 114 \$PI_BIN/);
    assert.equal(h.timeouts[3], 104_000 + 30_000 + 20_000);
    assert.match(session, /can errors after marengo-pi/);
    assert.match(session, /bin\/motor-repl disable/);

    const plan = JSON.parse(h.writes.get(`${CAL_DIR}/plan.json`) ?? "{}");
    assert.equal(plan.version, 1);
    assert.equal(plan.created_utc, "2026-10-03T12:00:00.000Z");
    assert.equal(plan.session_ts, TS);
    assert.equal(plan.profile, "bare_motor");
    assert.equal(plan.sweep_joint, "right_shoulder_pitch");
    assert.deepEqual(plan.fixed_rad, {});
    assert.deepEqual(plan.poses_rad, [0, 0.25, 0.48, 0.8, 1.2]);
    assert.equal(plan.approach_offset_rad, 0.05);
    assert.equal(plan.settle_sec, 2.5);
    assert.equal(plan.measure_sec, 1.5);
    assert.match(plan.gravity_gate_report, /PASS gravity gate/);
    assert.deepEqual(plan.steps[0], { joint: "right_shoulder_pitch", target_rad: -0.05, measure: false });
    assert.deepEqual(plan.steps[1], {
      joint: "right_shoulder_pitch",
      target_rad: 0,
      measure: true,
      pose_index: 0,
      approach: "below",
    });
    assert.deepEqual(plan.steps[6], { joint: "right_shoulder_pitch", target_rad: 1.25, measure: false });
    assert.deepEqual(plan.steps[7], {
      joint: "right_shoulder_pitch",
      target_rad: 1.2,
      measure: true,
      pose_index: 4,
      approach: "above",
    });
    assert.deepEqual(plan.steps.at(-1), {
      joint: "right_shoulder_pitch",
      target_rad: 0,
      measure: true,
      pose_index: 0,
      approach: "above",
    });
    assert.equal(plan.steps.length, 12);
    const steps = plan.steps as { joint: string; target_rad: number }[];
    assert.deepEqual(
      holdAts(session).slice(0, steps.length),
      steps.map((s) => `hold-at ${s.joint} ${String(s.target_rad)}`),
    );
    assert.equal(holdAts(session).length, steps.length + 1);

    assert.deepEqual(h.dirs, [`${CAL_DIR}/config`]);
    assert.equal(h.writes.get(`${CAL_DIR}/position-trace.csv`), TRACE_CSV);
    assert.equal(h.writes.get(`${CAL_DIR}/pi-marengo.urdf`), URDF);
    assert.equal(h.writes.get(`${CAL_DIR}/config/robot.yaml`), ROBOT_YAML);
    assert.equal(h.writes.get(`${CAL_DIR}/config/control.yaml`), controlYaml([-0.87, 2.9]));
    assert.equal(h.writes.get(`${CAL_DIR}/config/motors.yaml`), motorsYaml([-0.9, 2.92]));
    assert.match(h.writes.get(`${CAL_DIR}/bench-session.txt`) ?? "", /PASS gravity gate[\s\S]*"label":"gravity-calibrate"/);

    assert.deepEqual(h.fits, [
      {
        command: "cargo",
        args: [
          "run", "--release", "-q", "-p", "marengo-log-cli", "--", "gravity-fit", "--dir", CAL_DIR,
          "--fit", "mass:right_upper_arm", "--fit", "com:right_upper_arm",
        ],
        opts: { cwd: "/tmp/marengo", timeoutMs: 900_000 },
      },
    ]);
    assert.match(out, /gravity-fit: proposal written \(exit 0\)/);
    assert.match(out, /pi_sync_bench_urdf \(ADR 0017\)\. Never automatic\.$/);
    assert.deepEqual(h.audits, [{ tool: "pi_gravity_calibrate", exitCode: 0 }]);
  });

  it("elbow sweep moves the fixed pitch first and returns it last", async () => {
    const h = harness();
    await h.run({ ...OPT_INS, sweep_joint: "right_elbow_pitch", fixed_pitch_rad: 0.3, run_fit: false });
    const session = h.bodies[3];
    const lines = stdinLines(session);
    assert.equal(lines[3], "hold-at right_shoulder_pitch 0.3");
    assert.equal(lines[4], "hold-at right_elbow_pitch -0.05");
    assert.deepEqual(lines.slice(-5), [
      "hold-at right_elbow_pitch 0",
      "hold-at right_shoulder_pitch 0",
      "status",
      "disable",
      "quit",
    ]);
    assert.match(session, /"hold-at right_elbow_pitch 0";\nsleep 2;\nprintf '%s\\n' "hold-at right_shoulder_pitch 0";\nsleep 6;/);
    const plan = JSON.parse(h.writes.get(`${CAL_DIR}/plan.json`) ?? "{}");
    assert.deepEqual(plan.fixed_rad, { right_shoulder_pitch: 0.3 });
    assert.deepEqual(plan.steps[0], { joint: "right_shoulder_pitch", target_rad: 0.3, measure: false });
    assert.equal(plan.steps.length, 1 + 2 * 4 + 2);
    assert.deepEqual(h.fits, []);
  });

  it("skips the hanging-rest mismatch for calibration but reports it", async () => {
    const h = harness({ preview: gravityPreviewReply({ right_shoulder_pitch: 0.6 }) });
    const out = await h.run(OPT_INS);
    assert.match(out, /FAIL gravity_model_mismatch: \|τ_g\| at the hanging rest/);
    assert.match(out, /SKIPPED gravity_model_mismatch \(hanging rest\) for gravity calibration/);
    assert.equal(h.bodies.filter((b) => b.includes("LABEL='gravity-calibrate'")).length, 1);
  });

  it("still refuses an unavailable gravity preview", async () => {
    const h = harness({ preview: "error: model missing\n[exit 1]" });
    const out = await h.run(OPT_INS);
    assert.match(out, /FAIL gravity_gate_unavailable/);
    assert.equal(h.bodies.filter((b) => b.includes("LABEL='gravity-calibrate'")).length, 0);
    assert.deepEqual(h.audits, [{ tool: "pi_gravity_calibrate", exitCode: 1 }]);
  });

  it("returns the session output without files or fit when the session reports no trace", async () => {
    const h = harness({ session: "reference acquisition failed\n\n[exit 1]" });
    const out = await h.run(OPT_INS);
    assert.match(out, /reference acquisition failed/);
    assert.match(out, /no calibration files were written and no fit was run/);
    assert.equal(h.writes.size, 0);
    assert.deepEqual(h.fits, []);
    assert.equal(h.bodies.length, 4);
  });

  it("names a refused enable instead of a missing bench log line and fetches nothing", async () => {
    // 2026-10-03 15:34 bench: Enable admission refused; the session must stop before any
    // hold-at and say why.
    const h = harness({
      session: [
        `=== bench session ${TS} (gravity-calibrate) ===`,
        "homing verified → Ready",
        "enable failed: homing verify on right_shoulder_pitch: device identity reply missing at admission",
        "enable failed (enable bench); sending disable/quit",
        SESSION_JSON,
        "[exit 1]",
      ].join("\n"),
    });
    const out = await h.run(OPT_INS);
    assert.match(
      out,
      /marengo-pi refused the session before the sweep: enable failed: homing verify on right_shoulder_pitch: device identity reply missing at admission\n/,
    );
    assert.doesNotMatch(out, /no bench log\/trace line/);
    assert.equal(h.writes.size, 0);
    assert.deepEqual(h.fits, []);
    assert.equal(h.bodies.length, 4);
    assert.deepEqual(h.audits.at(-1), { tool: "pi_gravity_calibrate", exitCode: 1 });
  });

  it("awaits the readiness check and enable before the first hold-at", async () => {
    const h = harness();
    await h.run(OPT_INS);
    const session = h.bodies.find((b) => b.includes("LABEL='gravity-calibrate'")) ?? "";
    assert.match(
      session,
      /printf '%s\\n' "home"\n[\s\S]*grep -Eq '\^home failed:'[\s\S]*grep -q '\^homing verified '[\s\S]*printf '%s\\n' disable quit[\s\S]*printf '%s\\n' "enable bench"\n[\s\S]*grep -Eq '\^enable \(failed\|blocked\|refused\):'[\s\S]*grep -q '\^enabled \(operator='[\s\S]*printf '%s\\n' disable quit\n {2}exit 1\nfi;\nprintf '%s\\n' "hold-at /,
    );
  });

  it("keeps the capture but does not fit when the trace is missing", async () => {
    const h = harness({ trace: "[exit 1]" });
    const out = await h.run(OPT_INS);
    assert.match(out, /missing or without a header: no fit was run/);
    assert.ok(h.writes.has(`${CAL_DIR}/plan.json`));
    assert.ok(!h.writes.has(`${CAL_DIR}/position-trace.csv`));
    assert.deepEqual(h.fits, []);
  });

  it("reports a refused fit (exit 2) and a missing cargo with the exact command", async () => {
    const refused = harness({ fitExit: 2 });
    assert.match(await refused.run(OPT_INS), /gravity-fit refused the fit \(exit 2/);

    const missing = harness({ fitExit: 1, fitStderr: "spawn cargo ENOENT" });
    const out = await missing.run(OPT_INS);
    assert.match(out, /gravity-fit did not run \(exit 1, cargo not found\)/);
    assert.match(
      out,
      new RegExp(`Run it on the workstation: cd '/tmp/marengo' && cargo run --release -q -p marengo-log-cli -- gravity-fit --dir ${CAL_DIR}`),
    );
  });
});

describe("gravity calibration pure helpers", () => {
  it("extracts marked content verbatim", () => {
    assert.equal(extractMarked(marked("trace", "a\nb\n"), "trace"), "a\nb\n");
    assert.equal(extractMarked(marked("trace", "a"), "trace"), "a");
    assert.equal(extractMarked(marked("trace", ""), "trace"), "");
    assert.equal(extractMarked("nothing", "trace"), undefined);
  });

  it("parses nested mappings, item mappings and comments", () => {
    assert.deepEqual(parseYamlLite(motorsYaml([-0.9, 2.92]).split("\n").slice(0, 9).join("\n")), {
      motors: [
        {
          joint: "right_shoulder_pitch",
          driver: "robstride",
          firmware_version: "0.3.1.42",
          bench: { position_lower_rad: "-0.9", position_upper_rad: "2.92", torque_limit_nm: "5.0" },
        },
      ],
    });
  });

  it("never repeats a hold target on consecutive steps", () => {
    const plan = planCalibrationSweep({
      sweepJoint: "right_shoulder_pitch",
      posesRad: [1.2, 0, 0.48],
      fixedPitchRad: 0,
      approachOffsetRad: 0.05,
    });
    assert.ok(plan.ok);
    assert.deepEqual(plan.posesRad, [0, 0.48, 1.2]);
    for (let i = 1; i < plan.steps.length; i += 1) {
      assert.notDeepEqual(
        [plan.steps[i].joint, plan.steps[i].target_rad],
        [plan.steps[i - 1].joint, plan.steps[i - 1].target_rad],
      );
    }
    for (const s of plan.steps) assert.equal(Number(String(s.target_rad)), s.target_rad);
  });

  it("pre-flight shell reads config_dir YAML and the robot.yaml URDF verbatim", () => {
    const root = mkdtempSync(path.join(tmpdir(), "gravcal-"));
    mkdirSync(path.join(root, "config"));
    mkdirSync(path.join(root, "assets", "urdf"), { recursive: true });
    const files: Record<string, string> = {
      "config/robot.yaml": readFileSync(new URL("robot.yaml", REPO_CONFIG), "utf8"),
      "config/control.yaml": controlYaml([-0.87, 2.9]),
      "config/motors.yaml": motorsYaml([-0.9, 2.92]),
      "assets/urdf/marengo.urdf": URDF,
    };
    for (const [rel, data] of Object.entries(files)) writeFileSync(path.join(root, rel), data);
    const r = spawnSync("bash", ["-c", `set -euo pipefail\n${preflightReadShell()}`], {
      cwd: root,
      env: { ...process.env, MARENGO_ROOT: root, MARENGO_CONFIG_DIR: path.join(root, "config") },
      encoding: "utf8",
    });
    assert.equal(r.status, 0, r.stderr);
    const parsed = parsePreflight(r.stdout, root);
    assert.ok(parsed.ok, parsed.ok ? "" : parsed.message);
    assert.equal(parsed.robotYaml, files["config/robot.yaml"]);
    assert.equal(parsed.controlYaml, files["config/control.yaml"]);
    assert.equal(parsed.motorsYaml, files["config/motors.yaml"]);
    assert.equal(parsed.urdf, URDF);
    assert.equal(parsed.urdfPath, path.join(root, "assets/urdf/marengo.urdf"));
  });
});


describe("pi_gravity_calibrate session line from the shared bench wrapper", () => {
  it("parseSessionJson reads the trace and ts the generated shell prints", () => {
    const dir = mkdtempSync(path.join(tmpdir(), "grav-wrapper-"));
    const fake = path.join(dir, "fakebin");
    mkdirSync(fake);
    mkdirSync(path.join(dir, "bin"));
    const stubs: Record<string, string> = {
      "bin/motor-repl": "exit 0",
      "fakebin/sudo": "exit 0",
      "fakebin/systemctl": "echo inactive",
      "fakebin/pkill": "exit 1",
      "fakebin/pgrep": "exit 1",
      "fakebin/sleep": "exit 0",
      "fakebin/ip": "exit 0",
    };
    for (const [rel, body] of Object.entries(stubs)) {
      writeFileSync(path.join(dir, rel), `#!/bin/bash\n${body}\n`);
      chmodSync(path.join(dir, rel), 0o755);
    }
    const piCfg: MarengoPiConfig = { ...cfg, piRoot: dir };
    const script = benchLogWrapper(piCfg, "echo session-body", "gravity-calibrate", "/opt/marengo/config");
    const r = spawnSync("bash", ["-c", `{\n${script}\n} 2>&1`], {
      cwd: dir,
      encoding: "utf8",
      env: { ...process.env, MARENGO_CAN_SYSFS: path.join(dir, "sys"), PATH: `${fake}:${process.env.PATH ?? ""}` },
    });
    assert.equal(r.status, 0, r.stdout + r.stderr);
    const session = parseSessionJson(r.stdout);
    assert.ok(session, `no session line in:\n${r.stdout}`);
    assert.match(session.ts, /^\d{8}T\d{6}Z$/);
    assert.equal(session.trace, path.join(dir, "var", "log", `position-trace-${session.ts}.csv`));
  });
});
