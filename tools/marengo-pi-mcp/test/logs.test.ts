import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { MarengoPiConfig } from "../src/config.js";
import { registerLogTools } from "../src/tools/logs.js";
import { BENCH_LOG_KEEP_COUNT } from "../src/tools/motion.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config/bringup/arm_4dof_right",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

describe("log tools", () => {
  it("pi_logs_grep defaults last_files to 1 in remote script", async () => {
    let script = "";
    const tools = registerLogTools(cfg, async (body) => {
      script = body;
      return "";
    });

    await tools.pi_logs_grep.handler(
      tools.pi_logs_grep.inputSchema.parse({ pattern: "pos=1\\.7" }),
    );

    assert.match(script, /if \[\[ 1 -eq 1 \]\]; then/);
    assert.doesNotMatch(script, /undefined/);
    assert.match(
      script,
      /grep -E 'pos=1\\.7' "\$LOGDIR\/bench-latest.log"/,
    );
  });

  it("pi_logs_grep scans multiple files when last_files > 1", async () => {
    let script = "";
    const tools = registerLogTools(cfg, async (body) => {
      script = body;
      return "";
    });

    await tools.pi_logs_grep.handler({
      pattern: "fault",
      last_files: 3,
    });

    assert.match(script, /head -n 3/);
    assert.doesNotMatch(script, /if \[\[ 1 -eq 1 \]\]; then/);
  });

  it("BENCH_LOG_KEEP_COUNT is 50", () => {
    assert.equal(BENCH_LOG_KEEP_COUNT, 50);
  });

  it("pi_logs_tail defaults lines to 100", async () => {
    let script = "";
    const tools = registerLogTools(cfg, async (body) => {
      script = body;
      return "";
    });

    await tools.pi_logs_tail.handler(
      tools.pi_logs_tail.inputSchema.parse({}),
    );

    assert.match(script, /tail -n 100 /);
  });

  it("pi_candump_summary accepts leading whitespace in candump timestamps", async () => {
    let script = "";
    const tools = registerLogTools(cfg, async (body) => {
      script = body;
      return "";
    });

    await tools.pi_candump_summary.handler({});

    assert.match(script, /grep -m1 -E "\^\[\[:space:\]\]\*\\\("/);
    assert.match(script, /sed -n "s\/\^\[\[:space:\]\]\*\(\\\(\[0-9\.\]\*\\\)\)/);
    assert.match(script, /c=\$\{c:-0\}/);
    assert.match(script, /top CAN IDs/);
  });
});
