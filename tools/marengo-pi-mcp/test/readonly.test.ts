import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { MarengoPiConfig } from "../src/config.js";
import { registerReadonlyTools } from "../src/tools/readonly.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config/bringup/shoulder_pitch_right_only",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

describe("readonly CAN tools", () => {
  it("queries each CAN interface with valid ip syntax", async () => {
    let script = "";
    const tools = registerReadonlyTools(cfg, async (body) => {
      script = body;
      return body;
    });

    await tools.pi_can_status.handler();

    assert.match(script, /for iface in can0 can1; do/);
    assert.match(
      script,
      /ip -details -statistics link show dev "\$\{iface\}"/,
    );
    assert.doesNotMatch(script, /link show can0 can1/);
  });

  it("passes can0 and can1 as separate candump interfaces", async () => {
    let script = "";
    const tools = registerReadonlyTools(cfg, async (body) => {
      script = body;
      return body;
    });

    await tools.pi_candump_once.handler();

    assert.match(script, /timeout 2 candump -ta can0 can1/);
    assert.doesNotMatch(script, /can0,can1/);
  });

  it("pi_health runs homing preflight for active config", async () => {
    let script = "";
    const tools = registerReadonlyTools(cfg, async (body) => {
      script = body;
      return body;
    });

    await tools.pi_health.handler();

    assert.match(script, /homing-preflight\.sh/);
    assert.match(script, /shoulder_pitch_right_only/);
  });
});
