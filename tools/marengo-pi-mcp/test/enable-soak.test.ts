import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import type { MarengoPiConfig } from "../src/config.js";
import { MASTER_JOINTS } from "../src/bench-profiles.js";
import {
  type AnalyzerResult,
  type EnableSoakArgs,
  type EnableSoakDeps,
  type SoakCycle,
  CYCLE_FAULT_ERE,
  MAX_SOAK_SEC,
  MOTION_VERBS,
  SOAK_VERBS,
  evaluateSoak,
  parseFirmwareTiming,
  parseSoakCycles,
  planSoak,
  registerEnableSoakTools,
  soakCycleScript,
  soakSessionBody,
} from "../src/tools/enable-soak.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

const FIXTURE = readFileSync(new URL("./fixtures/firmware-timing.json", import.meta.url), "utf8");
const TS = "20261003T120000Z";
const SESSION_JSON = `{"log":"/opt/marengo/var/log/bench-${TS}.log","trace":"/opt/marengo/var/log/position-trace-${TS}.csv","candump":"/opt/marengo/var/log/candump-${TS}.log","ts":"${TS}","label":"enable-soak"}`;
const OPT_IN = { confirm: true, set_zero: true, at_mechanical_reference: true } as const;

function plan(input: Parameters<typeof planSoak>[0] = {}) {
  const p = planSoak(input);
  if (!p.ok) throw new Error(p.message);
  return p;
}

/** Synthetic session output for one cycle, as soakSessionBody + marengo-pi print it. */
function cycleText(
  i: number,
  n: number,
  opts: { enable?: string; extra?: string[]; over?: [number, number]; exit?: number; joints?: readonly string[] } = {},
): string {
  const [before, after] = opts.over ?? [5, 5];
  const exit = opts.exit ?? 0;
  return [
    `=== soak cycle ${i}/${n} begin ===`,
    `soak cycle ${i} can0 before rx_over_errors=${before} rx_errors=${before}`,
    "can settle: ok (0.5s quiet, no CAN owner) can0 rx_errors=5 rx_over=5 tx_errors=0",
    ...(opts.joints ?? MASTER_JOINTS).map((j) => `reference ${j} current pos=0.0000`),
    "homing verified → Ready",
    opts.enable ?? "enabled (operator=bench) targets=right_shoulder_pitch",
    ...(opts.enable === undefined ? ["soak enable_to_enabled_ms=812"] : []),
    ...(opts.extra ?? []),
    "right_shoulder_pitch (can0/id1): pos=-0.0000 rad vel=0.0 rad/s torque=0.0 Nm fault=0x0000 homing=3",
    "disabled",
    `soak cycle ${i} feeder=0 marengo_pi=${exit}`,
    `soak cycle ${i} can0 after rx_over_errors=${after} rx_errors=${after}`,
    `=== soak cycle ${i}/${n} end exit=${exit} ===`,
  ].join("\n");
}

function cleanSession(n: number): string {
  return [...Array.from({ length: n }, (_, k) => cycleText(k + 1, n)), "soak failed cycles: 0", SESSION_JSON].join("\n");
}

describe("pi_enable_soak refusals", () => {
  const tools = () => {
    const calls: string[] = [];
    const t = registerEnableSoakTools(cfg, async (body) => {
      calls.push(body);
      return "";
    }, () => {});
    return { t, calls };
  };

  it("refuses without confirm or the reference opt-ins, before contacting the Pi", async () => {
    const { t, calls } = tools();
    assert.match(await t.pi_enable_soak.handler({ confirm: false } as unknown as EnableSoakArgs), /^Motion blocked/);
    assert.match(await t.pi_enable_soak.handler({ confirm: true }), /set_zero: true and\s+at_mechanical_reference: true/);
    assert.match(await t.pi_enable_soak.handler({ confirm: true, set_zero: true }), /^Refused: enabling needs/);
    assert.equal(calls.length, 0);
  });

  it("refuses out-of-range cycles and dwell", async () => {
    const { t, calls } = tools();
    for (const cycles of [0, 51, 1.5]) {
      assert.match(await t.pi_enable_soak.handler({ ...OPT_IN, cycles }), /^Refused: cycles must be an integer in \[1, 50\]/);
    }
    for (const dwell_sec of [0.4, 10.5]) {
      assert.match(await t.pi_enable_soak.handler({ ...OPT_IN, dwell_sec }), /^Refused: dwell_sec must be in \[0\.5, 10\]/);
    }
    assert.equal(calls.length, 0);
  });

  it("refuses plans estimated over 480 s and accepts the 50-cycle default dwell", async () => {
    const { t, calls } = tools();
    const out = await t.pi_enable_soak.handler({ ...OPT_IN, cycles: 50, dwell_sec: 10 });
    assert.match(out, new RegExp(`^Refused: estimated soak duration \\d+ s .* exceeds ${MAX_SOAK_SEC} s`));
    assert.equal(calls.length, 0);
    assert.ok(plan({ cycles: 50, dwellSec: 2 }).estimatedSec <= MAX_SOAK_SEC);
    assert.equal(plan().estimatedSec, 20 + 20 * (6 + 2));
  });
});

describe("pi_enable_soak script", () => {
  it("sends exactly the no-motion cycle script for the profile joints", () => {
    const p = plan();
    assert.equal(p.profile, "arm_attached");
    assert.deepEqual(p.script, [
      `home ${MASTER_JOINTS.join(" ")} sign-tested`,
      "home",
      "enable bench",
      "sleep 2",
      "status",
      "disable",
      "quit",
    ]);
    assert.deepEqual(plan({ profile: "roll_attached" }).joints, MASTER_JOINTS.slice(0, 3));
  });

  it("contains no motion verb, in the script or in the generated remote command", () => {
    const p = plan({ cycles: 3 });
    for (const line of p.script) {
      const verb = line.split(/\s+/)[0];
      assert.ok((SOAK_VERBS as readonly string[]).includes(verb), line);
      assert.ok(!(MOTION_VERBS as readonly string[]).includes(verb), line);
    }
    const body = soakSessionBody(cfg, p);
    const sent = [...body.matchAll(/printf '%s\\n' "([^"]*)"/g)].map((m) => m[1]);
    assert.ok(sent.length > 0);
    for (const line of sent) {
      assert.ok((SOAK_VERBS as readonly string[]).includes(line.split(/\s+/)[0]), line);
    }
    assert.doesNotMatch(body, /hold|gravity|torque|impedance|wave/);
  });

  it("rejects a motion verb if one ever reaches the cycle script", () => {
    assert.throws(() => soakCycleScript(["right_shoulder_pitch"], "hold-on", 2), /not a no-motion verb/);
  });

  it("launches every soak marengo-pi without the Chappe IPC socket", () => {
    const body = soakSessionBody(cfg, plan({ cycles: 2 }));
    const launches = body.match(/\| timeout \d+ [^\n]*\$PI_BIN"?/g) ?? [];
    assert.equal(launches.length, 1, "one launch inside the per-cycle loop");
    assert.match(launches[0], /\| timeout \d+ env -u MARENGO_CHAPPE_SOCKET "\$PI_BIN"$/);
  });

  it("exports the joint subset only for subset profiles", () => {
    assert.match(soakSessionBody(cfg, plan({ profile: "roll_attached" })), /^export MARENGO_JOINT_SUBSET=right_shoulder_pitch,right_shoulder_roll,right_upper_arm_yaw$/m);
    assert.doesNotMatch(soakSessionBody(cfg, plan()), /MARENGO_JOINT_SUBSET/);
  });
});

/**
 * Run soakSessionBody against a fake bin/marengo-pi (answers on stdout like marengo-pi, records
 * stdin and whether MARENGO_CHAPPE_SOCKET reached it), instant sleep, pass-through timeout,
 * silent ip/pgrep and sysfs CAN counters under MARENGO_CAN_SYSFS.
 */
function runSoakBody(
  opts: { cycles?: number; stopOnFault?: boolean; enableReply?: (cycle: number) => string; overrunPerCycle?: boolean } = {},
) {
  const dir = mkdtempSync(path.join(tmpdir(), "soak-body-"));
  const fake = path.join(dir, "fakebin");
  const stats = path.join(dir, "sys", "can0", "statistics");
  mkdirSync(path.join(dir, "bin"));
  mkdirSync(fake);
  mkdirSync(stats, { recursive: true });
  for (const counter of ["rx_errors", "rx_over_errors", "tx_errors"]) writeFileSync(path.join(stats, counter), "0\n");
  const counterFile = path.join(dir, "cycle");
  writeFileSync(counterFile, "0\n");
  const replies = Array.from({ length: opts.cycles ?? 2 }, (_, k) =>
    opts.enableReply?.(k + 1) ?? "enabled (operator=bench) targets=right_shoulder_pitch",
  );
  const scripts: Record<string, string> = {
    "bin/marengo-pi": [
      `n=$(( $(cat ${counterFile}) + 1 )); echo $n > ${counterFile}`,
      `echo "cycle $n chappe=\${MARENGO_CHAPPE_SOCKET-unset}" >> ${path.join(dir, "trace")}`,
      ...(opts.overrunPerCycle
        ? [`f=${path.join(stats, "rx_over_errors")}; echo $(( $(cat "$f") + 1 )) > "$f"`]
        : []),
      `replies=(${replies.map((r) => JSON.stringify(r)).join(" ")})`,
      "while IFS= read -r l; do",
      `  printf '%s %s\\n' "$n" "$l" >> ${path.join(dir, "stdin")}`,
      '  case "$l" in',
      '    *sign-tested) for j in ${l#home }; do [[ $j == sign-tested ]] || echo "reference $j current pos=0.0000"; done ;;',
      '    home) echo "homing verified → Ready" ;;',
      '    enable*) echo "${replies[$((n - 1))]}" ;;',
      '    status) echo "right_shoulder_pitch (can0/id1): pos=0.0000 rad fault=0x0000 homing=3" ;;',
      '    disable) echo "disabled" ;;',
      "    quit) break ;;",
      "  esac",
      "done",
    ].join("\n"),
    "fakebin/sleep": "exit 0",
    "fakebin/timeout": 'shift; exec "$@"',
    "fakebin/ip": "exit 0",
    "fakebin/pgrep": "exit 1",
  };
  for (const [rel, body] of Object.entries(scripts)) {
    writeFileSync(path.join(dir, rel), `#!/bin/bash\n${body}\n`);
    chmodSync(path.join(dir, rel), 0o755);
  }
  writeFileSync(path.join(dir, "trace"), "");
  writeFileSync(path.join(dir, "stdin"), "");
  const p = plan({ cycles: opts.cycles ?? 2, stopOnFault: opts.stopOnFault });
  const r = spawnSync("bash", ["-c", `set -uo pipefail\n{\n${soakSessionBody(cfg, p)}\n} 2>&1`], {
    cwd: dir,
    env: {
      ...process.env,
      MARENGO_CHAPPE_SOCKET: "/run/marengo/chappe.sock",
      MARENGO_CAN_SYSFS: path.join(dir, "sys"),
      TMPDIR: dir,
      PATH: `${fake}:${process.env.PATH ?? ""}`,
    },
    encoding: "utf8",
    timeout: 60_000,
  });
  return {
    status: r.status,
    output: `${r.stdout}\n${r.stderr}`,
    trace: readFileSync(path.join(dir, "trace"), "utf8").trim().split("\n").filter(Boolean),
    stdin: readFileSync(path.join(dir, "stdin"), "utf8").trim().split("\n").filter(Boolean),
    plan: p,
  };
}

describe("pi_enable_soak session body (fake marengo-pi)", () => {
  it("runs a fresh marengo-pi per cycle with the exact script and no Chappe socket", () => {
    const r = runSoakBody({ cycles: 2 });
    assert.equal(r.status, 0, r.output);
    assert.deepEqual(r.trace, ["cycle 1 chappe=unset", "cycle 2 chappe=unset"]);
    const expected = r.plan.script.filter((l) => !l.startsWith("sleep "));
    assert.deepEqual(r.stdin, [...expected.map((l) => `1 ${l}`), ...expected.map((l) => `2 ${l}`)]);
    const cycles = parseSoakCycles(r.output, MASTER_JOINTS.length);
    assert.equal(cycles.length, 2);
    for (const c of cycles) {
      assert.equal(c.clean, true, JSON.stringify(c));
      assert.equal(typeof c.enableMs, "number");
      assert.deepEqual(c.can0Before, { rxOver: 0, rxErrors: 0 });
      assert.deepEqual(c.can0After, { rxOver: 0, rxErrors: 0 });
    }
  });

  it("ends a refused cycle with disable/quit, records it and continues", () => {
    const r = runSoakBody({
      cycles: 2,
      enableReply: (n) =>
        n === 1 ? "enable failed: homing verify on right_shoulder_pitch: device identity reply missing at admission" : "enabled (operator=bench) targets=x",
    });
    assert.equal(r.status, 1);
    assert.ok(!r.stdin.includes("1 status"), "no status after a refused enable");
    assert.ok(r.stdin.includes("1 disable") && r.stdin.includes("1 quit"));
    const [first, second] = parseSoakCycles(r.output, MASTER_JOINTS.length);
    assert.equal(first.clean, false);
    assert.equal(first.exit, 1);
    // marengo-pi's refusal, then the feeder's own `enable failed (…); sending disable/quit`.
    assert.deepEqual(first.faultKinds, ["homing_verify", "enable_refused"]);
    assert.match(first.enableLine ?? "", /^enable failed: homing verify/);
    assert.equal(second.clean, true);
  });

  it("stop_on_fault stops after the first unclean cycle", () => {
    const r = runSoakBody({ cycles: 3, stopOnFault: true, enableReply: () => "enable blocked: homing: not Ready" });
    assert.equal(r.status, 1);
    assert.match(r.output, /soak stop_on_fault: stopping after cycle 1/);
    assert.equal(parseSoakCycles(r.output, MASTER_JOINTS.length).length, 1);
  });

  it("records the can0 overrun increase per cycle", () => {
    const r = runSoakBody({ cycles: 2, overrunPerCycle: true });
    const cycles = parseSoakCycles(r.output, MASTER_JOINTS.length);
    assert.deepEqual(cycles.map((c) => [c.can0Before?.rxOver, c.can0After?.rxOver]), [[0, 1], [1, 2]]);
    const verdict = evaluateSoak(cycles, 2, { status: "unavailable", reason: "test" });
    assert.equal(verdict.pass, false);
    assert.deepEqual(verdict.reasons, ["can0 rx_over_errors +2"]);
  });
});

describe("pi_enable_soak parsing", () => {
  it("classifies fault and refusal lines", () => {
    const out = [
      cycleText(1, 3, {
        extra: [
          "2026-10-03T14:10:27.068799Z ERROR run_control_loop: marengo_pi: control tick failed error=safety: persistent safety fault: Transport: rx overflow",
        ],
        exit: 0,
      }),
      cycleText(2, 3, { extra: ["2026-10-03T14:10:27Z  WARN davout: DriveState mismatch on right_elbow_pitch"] }),
      cycleText(3, 3, {
        extra: ["right_elbow_pitch (can0/id4): pos=0.0 rad fault=0x0080 homing=3"],
        joints: MASTER_JOINTS.slice(0, 4),
      }),
    ].join("\n");
    const [a, b, c] = parseSoakCycles(out, MASTER_JOINTS.length);
    assert.deepEqual(a.faultKinds, ["Transport"]);
    assert.deepEqual(b.faultKinds, ["DriveState"]);
    assert.deepEqual(c.faultKinds, ["drive_fault"]);
    assert.equal(c.referencesAcquired, 4);
    for (const cycle of [a, b, c]) assert.equal(cycle.clean, false);
  });

  it("does not flag clean status, settle or INFO lines", () => {
    const out = cycleText(1, 1, {
      extra: ["2026-10-03T15:35:03.998684Z  INFO marengo_pi: owner shutdown outcome persist_idle=true status=Complete"],
    });
    const [c] = parseSoakCycles(out, MASTER_JOINTS.length);
    assert.deepEqual(c.faults, []);
    assert.equal(c.clean, true);
    assert.equal(c.enableMs, 812);
  });

  it("shares one fault pattern between grep -E and JavaScript", () => {
    const lines = ["home failed: x", "2026-10-03T00:00:00Z ERROR y", "z fault=0x0000", "z fault=0x0100"];
    const r = spawnSync("grep", ["-E", CYCLE_FAULT_ERE], { input: `${lines.join("\n")}\n`, encoding: "utf8" });
    const js = lines.filter((l) => new RegExp(CYCLE_FAULT_ERE).test(l));
    assert.deepEqual(r.stdout.trim().split("\n"), js);
    assert.deepEqual(js, ["home failed: x", "2026-10-03T00:00:00Z ERROR y", "z fault=0x0100"]);
  });
});

describe("pi_enable_soak verdict", () => {
  const ok = (json = FIXTURE): AnalyzerResult => {
    const parsed = parseFirmwareTiming(json);
    if (!parsed.ok) throw new Error(parsed.message);
    return { status: "ok", timing: parsed.timing };
  };
  const cycles = (n: number): SoakCycle[] => parseSoakCycles(cleanSession(n), MASTER_JOINTS.length);

  it("passes only when every cycle is clean, overruns held and the wire stayed neutral", () => {
    assert.deepEqual(evaluateSoak(cycles(3), 3, ok()).reasons, []);
    assert.equal(evaluateSoak(cycles(3), 3, ok()).pass, true);
    assert.equal(evaluateSoak(cycles(3), 3, { status: "unavailable", reason: "x" }).pass, true);
    assert.deepEqual(evaluateSoak(cycles(2), 3, ok()).reasons, ["2/3 cycles ran"]);
    assert.equal(evaluateSoak([], 1, ok()).pass, false);
  });

  it("non_neutral_mit > 0 fails regardless of clean cycles", () => {
    const json = JSON.stringify({ ...JSON.parse(FIXTURE), non_neutral_mit: { count: 3, first_s: 1.2, last_s: 1.4 } });
    const verdict = evaluateSoak(cycles(3), 3, ok(json));
    assert.equal(verdict.pass, false);
    assert.deepEqual(verdict.reasons, ["non_neutral_mit=3 on the wire"]);
  });

  it("fails an unclean cycle and counts fault kinds", () => {
    const out = [cycleText(1, 2), cycleText(2, 2, { enable: "enable failed: timeout", exit: 1 })].join("\n");
    const verdict = evaluateSoak(parseSoakCycles(out, MASTER_JOINTS.length), 2, ok());
    assert.equal(verdict.pass, false);
    assert.deepEqual(verdict.faultKinds, { enable_refused: 1 });
    assert.deepEqual(verdict.reasons, ["1 unclean cycle(s)"]);
  });

  it("parses the analyzer contract and rejects anything else", () => {
    const parsed = parseFirmwareTiming(FIXTURE);
    assert.ok(parsed.ok);
    assert.equal(parsed.timing.drives["can0/2"].mit_unanswered, 1);
    const notJson = parseFirmwareTiming("error: unrecognized subcommand");
    assert.ok(!notJson.ok);
    assert.match(notJson.message, /not JSON/);
    const missing = JSON.parse(FIXTURE);
    delete missing.bus;
    const noBus = parseFirmwareTiming(JSON.stringify(missing));
    assert.ok(!noBus.ok);
    assert.match(noBus.message, /contract: bus/);
  });
});

describe("pi_enable_soak handler", () => {
  function harness(opts: { sessionOut: string; cargo?: { stdout: string; stderr?: string; exitCode: number } }) {
    const execs: { command: string; args: string[] }[] = [];
    const files: Record<string, string> = {};
    const audits: number[] = [];
    let remoteTimeout = 0;
    const deps: EnableSoakDeps = {
      execLocal: async (command, args) => {
        execs.push({ command, args });
        if (command === "cargo") return { stderr: "", ...(opts.cargo ?? { stdout: FIXTURE, exitCode: 0 }) };
        return { stdout: "", stderr: "", exitCode: 0 };
      },
      writeFile: async (file, data) => {
        files[file] = data;
      },
      mkdir: async () => {},
      now: () => new Date("2026-10-03T12:00:00Z"),
    };
    const tools = registerEnableSoakTools(
      cfg,
      async (_body, timeoutMs) => {
        remoteTimeout = timeoutMs ?? 0;
        return opts.sessionOut;
      },
      (_tool, _args, _text, exit) => audits.push(exit),
      deps,
    );
    return { tools, execs, files, audits, remoteTimeout: () => remoteTimeout };
  }

  it("copies the session, runs firmware-timing and reports PASS", async () => {
    const h = harness({ sessionOut: cleanSession(2) });
    const out = await h.tools.pi_enable_soak.handler({ ...OPT_IN, cycles: 2 });
    const dir = path.join(cfg.localRoot, "var", "enable-soak", TS);
    assert.match(out, /\| 1 \| 5\/5 \| enabled \| 812 \| - \| 0 \| 0 \| 0 \|/);
    assert.match(out, /clean cycles: 2\/2/);
    assert.match(out, /non_neutral_mit=0/);
    assert.match(out, /can0\/2: enable→run ms n=2 p50=2 p95=3\.9 max=4/);
    assert.match(out, /VERDICT: PASS$/);
    assert.deepEqual(
      h.execs.map((e) => [e.command, e.args.at(-2), e.args.at(-1)]),
      [
        ["scp", `joey@marengo.local:/opt/marengo/var/log/bench-${TS}.log`, path.join(dir, "bench-session.log")],
        ["scp", `joey@marengo.local:/opt/marengo/var/log/candump-${TS}.log`, path.join(dir, "candump.log")],
        ["cargo", "--json", path.join(dir, "candump.log")],
      ],
    );
    assert.ok(h.execs[2].args.includes("firmware-timing"));
    assert.deepEqual(Object.keys(h.files).sort(), [path.join(dir, "firmware-timing.json"), path.join(dir, "soak-summary.txt")]);
    assert.deepEqual(h.audits, [0]);
    assert.ok(h.remoteTimeout() > 2 * 60_000);
  });

  it("FAILs on non-neutral MIT from the analyzer", async () => {
    const json = JSON.stringify({ ...JSON.parse(FIXTURE), non_neutral_mit: { count: 1, first_s: 3.5, last_s: 3.5 } });
    const h = harness({ sessionOut: cleanSession(1), cargo: { stdout: json, exitCode: 0 } });
    const out = await h.tools.pi_enable_soak.handler({ ...OPT_IN, cycles: 1 });
    assert.match(out, /VERDICT: FAIL — non_neutral_mit=1 on the wire$/);
    assert.deepEqual(h.audits, [1]);
  });

  it("reports a missing analyzer without inventing a wire check", async () => {
    const h = harness({
      sessionOut: cleanSession(1),
      cargo: { stdout: "", stderr: "error: unrecognized subcommand 'firmware-timing'", exitCode: 2 },
    });
    const out = await h.tools.pi_enable_soak.handler({ ...OPT_IN, cycles: 1 });
    assert.match(out, /not run: firmware-timing exit 2: error: unrecognized subcommand/);
    assert.match(out, /VERDICT: PASS$/);
  });

  it("FAILs when no cycle ran and keeps the session output locally", async () => {
    const h = harness({ sessionOut: "error: marengo-pi (pid 7) still owns CAN; refusing to open a second owner\n[exit 1]" });
    const out = await h.tools.pi_enable_soak.handler({ ...OPT_IN, cycles: 1 });
    assert.match(out, /No soak cycle ran/);
    assert.match(out, /VERDICT: FAIL — 0\/1 cycles ran/);
    const dir = path.join(cfg.localRoot, "var", "enable-soak", TS);
    assert.match(h.files[path.join(dir, "bench-session.log")] ?? "", /still owns CAN/);
    assert.ok(!h.execs.some((e) => e.command === "cargo"));
  });
});
