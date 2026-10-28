import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import type { MarengoPiConfig } from "../src/config.js";
import {
  managementGateShell,
  managementRefusal,
} from "../src/management-gate.js";
import { registerAdminTools } from "../src/tools/admin.js";
import { runRestartMarengoPi } from "../src/tools/restart-marengo-pi.js";
import { defaultLocalRoot } from "../src/config.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config",
  localRoot: defaultLocalRoot(),
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

function varint(n: number): number[] {
  const out: number[] = [];
  do {
    let byte = n % 128;
    n = Math.floor(n / 128);
    if (n > 0) byte |= 0x80;
    out.push(byte);
  } while (n > 0);
  return out;
}

const NOW = 1_761_660_000_000;
const b64 = (bytes: number[]) => Buffer.from(bytes).toString("base64");
const safety = (mode: number) => b64([0x08, ...varint(NOW - 100), ...(mode ? [0x10, mode] : [])]);
const heartbeat = (ts: number) => b64([0x08, ...varint(ts)]);

/** The gateway's `GET /config/snapshot` body, reduced to its config save state. */
const SAVED = { persist_ok: true, persist_pending: false };

function piOutput(opts: {
  marengoPi?: number;
  motorRepl?: number;
  mode?: number | null;
  heartbeatMs?: number | null;
  config?: Record<string, unknown> | null;
}): string {
  const mode = opts.mode === undefined ? 1 : opts.mode;
  const hb = opts.heartbeatMs === undefined ? NOW - 200 : opts.heartbeatMs;
  const config = opts.config === undefined ? SAVED : opts.config;
  return [
    `mgmt_marengo_pi=${opts.marengoPi ?? 1}`,
    `mgmt_motor_repl=${opts.motorRepl ?? 0}`,
    `mgmt_safety_b64=${mode === null ? "" : safety(mode)}`,
    `mgmt_heartbeat_b64=${hb === null ? "" : heartbeat(hb)}`,
    `mgmt_config_b64=${config === null ? "" : Buffer.from(JSON.stringify({ profile: "master", ...config })).toString("base64")}`,
    `mgmt_now_ms=${NOW}`,
  ].join("\n");
}

describe("management gate refuses while a config save is pending or failed (gateway persist gate)", () => {
  it("refuses a pending or failed save while marengo-pi runs Disabled", () => {
    assert.match(
      managementRefusal(piOutput({ config: { persist_ok: true, persist_pending: true } }), "pi_build") ?? "",
      /pi_build refused: .*save is still pending/,
    );
    assert.match(
      managementRefusal(piOutput({ config: { persist_ok: false, persist_pending: false } }), "pi_build") ?? "",
      /pi_build refused: .*save failed/,
    );
  });

  it("refuses a known pending or failed save when marengo-pi is not running", () => {
    const stopped = { marengoPi: 0, mode: null, heartbeatMs: null };
    assert.match(
      managementRefusal(piOutput({ ...stopped, config: { persist_ok: false, persist_pending: false } }), "x") ?? "",
      /save failed/,
    );
    assert.match(
      managementRefusal(piOutput({ ...stopped, config: { persist_ok: true, persist_pending: true } }), "x") ?? "",
      /pending/,
    );
  });

  it("fails closed on an unknown save state while marengo-pi runs", () => {
    assert.match(managementRefusal(piOutput({ config: null }), "x") ?? "", /save state is unknown/);
    // A gateway without the pending field (older build) is unknown too.
    assert.match(managementRefusal(piOutput({ config: { persist_ok: true } }), "x") ?? "", /save state is unknown/);
    assert.match(managementRefusal(piOutput({ config: { persist_ok: "yes", persist_pending: false } }), "x") ?? "", /unknown/);
  });

  it("allows an unknown save state when marengo-pi is not running (nothing live to revert)", () => {
    assert.equal(managementRefusal(piOutput({ marengoPi: 0, mode: null, heartbeatMs: null, config: null }), "x"), undefined);
  });

  it("the remote shell reads the save state with the Pi's gateway credential", () => {
    const answer = Buffer.from(JSON.stringify({ persist_ok: true, persist_pending: true })).toString("base64");
    // curl stub: the config snapshot answers only to the bearer credential from /etc/marengo/env.
    const curl = [
      "curl() {",
      '  local url="${*: -1}" hdr="" prev=""',
      '  for a in "$@"; do [[ "$prev" == -H && "$a" == @* ]] && hdr="$(cat "${a#@}")"; prev="$a"; done',
      `  [[ "$url" == */config/snapshot && "$hdr" == "Authorization: Bearer cfg-secret" ]] && { echo ${answer} | base64 -d; return 0; }`,
      "  return 22",
      "}",
    ].join("\n");
    const r = spawnSync(
      "bash",
      ["-c", `set -euo pipefail\npgrep() { echo 0; return 1; }\n${curl}\nMARENGO_GATEWAY_CONFIG_TOKEN=cfg-secret\n${managementGateShell()}`],
      { encoding: "utf8" },
    );
    assert.equal(r.status, 0, r.stderr);
    assert.match(managementRefusal(r.stdout, "x") ?? "", /pending/);
  });

  it("pi_build never reaches install-pi.sh while a save is pending", async () => {
    const bodies: string[] = [];
    const run = async (body: string) => {
      bodies.push(body);
      return body.includes("mgmt_marengo_pi=") ? piOutput({ config: { persist_ok: true, persist_pending: true } }) : "[exit 0]";
    };
    const out = await registerAdminTools(cfg, run).pi_build.handler();
    assert.match(out, /refused/);
    assert.equal(bodies.length, 1, "only the gate read ran");
  });
});

describe("a management refusal is an MCP error (isError)", () => {
  it("every refusal carries the Error: prefix the server flags", () => {
    const refusals = [
      piOutput({ mode: 3 }),
      piOutput({ mode: null }),
      piOutput({ heartbeatMs: null }),
      piOutput({ motorRepl: 1 }),
      piOutput({ config: null }),
      piOutput({ config: { persist_ok: false, persist_pending: false } }),
      "ssh: connect timed out",
    ].map((out) => managementRefusal(out, "pi_sync_main"));
    for (const text of refusals) assert.match(text ?? "", /^Error: pi_sync_main refused: /);
  });
});

describe("management gate mirrors the gateway (P2B-05)", () => {
  it("allows when marengo-pi is not running", () => {
    assert.equal(managementRefusal(piOutput({ marengoPi: 0, mode: null, heartbeatMs: null }), "x"), undefined);
  });

  it("allows Disabled and Ready with a fresh heartbeat", () => {
    assert.equal(managementRefusal(piOutput({ mode: 1 }), "x"), undefined);
    assert.equal(managementRefusal(piOutput({ mode: 2 }), "x"), undefined);
  });

  it("refuses Active", () => {
    assert.match(managementRefusal(piOutput({ mode: 3 }), "pi_build") ?? "", /pi_build refused: .*mode Active/);
  });

  it("refuses an unknown mode, a missing snapshot, or a stale heartbeat", () => {
    assert.match(managementRefusal(piOutput({ mode: 0 }), "x") ?? "", /mode Unspecified/);
    assert.match(managementRefusal(piOutput({ mode: null }), "x") ?? "", /mode is unknown/);
    assert.match(managementRefusal(piOutput({ heartbeatMs: null }), "x") ?? "", /no heartbeat/);
    assert.match(managementRefusal(piOutput({ heartbeatMs: NOW - 5_001 }), "x") ?? "", /stale/);
    assert.match(managementRefusal(piOutput({ heartbeatMs: NOW + 1 }), "x") ?? "", /stale/);
  });

  it("refuses while motor-repl runs, and when the process table is unreadable", () => {
    assert.match(managementRefusal(piOutput({ marengoPi: 0, motorRepl: 1 }), "x") ?? "", /motor-repl/);
    assert.match(managementRefusal("ssh: connect timed out", "x") ?? "", /process table/);
  });

  it("the remote shell prints every marker under strict mode with nothing running", () => {
    const r = spawnSync(
      "bash",
      ["-c", `set -euo pipefail\npgrep() { echo 0; return 1; }\ncurl() { return 7; }\n${managementGateShell()}`],
      { encoding: "utf8" },
    );
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /^mgmt_marengo_pi=0$/m);
    assert.match(r.stdout, /^mgmt_now_ms=\d+$/m);
    assert.equal(managementRefusal(r.stdout, "x"), undefined);
  });
});

describe("management tools refuse while the arm is Active", () => {
  function recorder(gateOutput: string) {
    const bodies: string[] = [];
    const run = async (body: string) => {
      bodies.push(body);
      return body.includes("mgmt_marengo_pi=") ? gateOutput : "[exit 0]";
    };
    return { bodies, run };
  }

  for (const tool of ["pi_build", "pi_install_staging", "pi_sync_main"] as const) {
    it(`${tool} never reaches install-pi.sh`, async () => {
      const { bodies, run } = recorder(piOutput({ mode: 3 }));
      const tools = registerAdminTools(cfg, run);
      const out =
        tool === "pi_sync_main"
          ? await tools.pi_sync_main.handler({ strategy: "pi_native", wait_for_ready: false, wait_timeout_sec: 30 })
          : await tools[tool].handler();
      assert.match(out, /refused: marengo-pi is running in mode Active/);
      assert.equal(bodies.length, 1, "only the gate read ran");
    });
  }

  it("pi_restart_marengo_pi restart is refused, stop is not", async () => {
    const active = recorder(piOutput({ mode: 3 }));
    const refused = await runRestartMarengoPi(cfg, active.run, { confirm: true, mode: "restart" });
    assert.match(refused, /refused/);
    assert.equal(active.bodies.length, 1);

    const stop = recorder(piOutput({ mode: 3 }));
    await runRestartMarengoPi(cfg, stop.run, { confirm: true, mode: "stop" });
    assert.equal(stop.bodies.length, 1);
    assert.match(stop.bodies[0], /pi-restart-marengo-pi\.sh/);
    assert.doesNotMatch(stop.bodies[0], /mgmt_marengo_pi=/);
  });

  it("runs the action when marengo-pi is Disabled", async () => {
    const { bodies, run } = recorder(piOutput({ mode: 1 }));
    await registerAdminTools(cfg, run).pi_install_staging.handler();
    assert.equal(bodies.length, 2);
    assert.match(bodies[1], /install-pi\.sh/);
  });
});
