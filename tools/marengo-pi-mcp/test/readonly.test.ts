import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { MarengoPiConfig } from "../src/config.js";
import { registerReadonlyTools } from "../src/tools/readonly.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};


describe("readonly CAN tools", () => {
  it("discloses active-reporting CAN writes and that streams remain on", () => {
    const tools = registerReadonlyTools(cfg, async () => "");

    assert.match(tools.pi_motor_repl_status.description, /type-24.*active-reporting/i);
    assert.match(tools.pi_motor_repl_status.description, /does not send.*off/i);
    assert.match(tools.pi_gravity_preview.description, /type-24.*active-reporting/i);
    assert.match(tools.pi_gravity_preview.description, /does not send.*off/i);
  });

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

  for (const [tool, command] of [
    ["pi_motor_repl_status", "bin/motor-repl status"],
    ["pi_gravity_preview", "bin/motor-repl gravity-preview"],
  ] as const) {
    it(`${tool} runs ${command} only when no process owns CAN`, async () => {
      let script = "";
      const tools = registerReadonlyTools(cfg, async (body) => {
        script = body;
        return body;
      });

      await tools[tool].handler({});

      assert.match(script, /pgrep -l -x 'marengo-pi\|motor-repl'/);
      const runs = (branch: string) => branch.split("\n").some((line) => line.trim() === command);
      const ownedStart = script.indexOf('if [[ -n "$CAN_OWNER" ]]; then');
      const elseAt = script.indexOf("\nelse\n", ownedStart);
      assert.ok(ownedStart >= 0 && elseAt > ownedStart, "owner branch present");
      assert.ok(!runs(script.slice(ownedStart, elseAt)), `${command} absent while CAN owned`);
      assert.ok(runs(script.slice(elseAt)), `${command} runs when CAN free`);
    });
  }

  it("accepts no pose or a full robot.yaml-order gravity pose only", async () => {
    let script = "";
    const tools = registerReadonlyTools(cfg, async (body) => {
      script = body;
      return body;
    });
    const schema = tools.pi_gravity_preview.inputSchema;

    assert.equal(schema.safeParse({}).success, true);
    assert.equal(schema.safeParse({ angles: [0, 1, 2, 3, 4] }).success, true);
    assert.equal(schema.safeParse({ angles: [0, 1] }).success, false);

    await tools.pi_gravity_preview.handler({ angles: [0, 1, 2, 3, 4] });
    assert.match(script, /bin\/motor-repl gravity-preview 0 1 2 3 4/);
  });

  it("pi_can_status and pi_candump_once never start motor-repl", async () => {
    const scripts: string[] = [];
    const tools = registerReadonlyTools(cfg, async (body) => {
      scripts.push(body);
      return body;
    });

    await tools.pi_can_status.handler();
    await tools.pi_candump_once.handler();

    for (const script of scripts) assert.doesNotMatch(script, /motor-repl|marengo-pi /);
  });
});
