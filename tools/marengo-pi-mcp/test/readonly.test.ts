import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { MarengoPiConfig } from "../src/config.js";
import { registerReadonlyTools, renderProtocolInspection } from "../src/tools/readonly.js";

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
  it("documents read-only motor-repl CAN ownership accurately", () => {
    const tools = registerReadonlyTools(cfg, async () => "");

    assert.match(tools.pi_motor_repl_status.description, /opens CAN.*bypasses Davout Supervisor/i);
    assert.match(tools.pi_motor_repl_status.description, /sends no type-24/i);
    assert.doesNotMatch(tools.pi_motor_repl_status.description, /does not send.*off/i);
    assert.match(tools.pi_gravity_preview.description, /does not open CAN/i);
    assert.doesNotMatch(tools.pi_gravity_preview.description, /type-24.*active-reporting/i);
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
    ["pi_protocol_inspect", "bin/motor-repl protocol-inspect"],
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

  it("pi_protocol_inspect passes only known joints, quoted", async () => {
    let script = "";
    const tools = registerReadonlyTools(cfg, async (body) => {
      script = body;
      return body;
    });
    const schema = tools.pi_protocol_inspect.inputSchema;

    assert.equal(schema.safeParse({}).success, true);
    assert.equal(schema.safeParse({ joints: ["right_elbow_pitch"] }).success, true);
    assert.equal(schema.safeParse({ joints: ["right_elbow_pitch; reboot"] }).success, false);
    assert.equal("confirm" in schema.shape, false, "read-only: no confirmation");

    await tools.pi_protocol_inspect.handler({ joints: ["right_elbow_pitch", "right_lower_arm_yaw"] });
    assert.match(
      script,
      /bin\/motor-repl protocol-inspect 'right_elbow_pitch' 'right_lower_arm_yaw'/,
    );
    assert.doesNotMatch(script, /pi-restart-marengo-pi|systemctl stop|pkill/);
  });

  it("summarizes per-joint firmware and CanTimeout above the raw output", () => {
    const raw = [
      "inspect right_shoulder_pitch can0:1 firmware=0.3.1.42 uid=457b30020c323817 run_mode=0 mech_pos=0.0100 mech_vel=0.0000 can_timeout=600 (0.030 s) zero_sta=1 add_offset=0.0000",
      "inspect right_lower_arm_yaw can0:5 firmware=0.0.3.32 uid=0102030405060708 run_mode=0 mech_pos=0.0000 mech_vel=0.0000 can_timeout=0 (off) zero_sta=1 add_offset=0.0000",
      "protocol-inspect: 2 drives read, all drives Disabled again (SocketCAN motors.yaml)",
    ].join("\n");

    const rendered = renderProtocolInspection(raw);

    assert.match(rendered, /\| right_shoulder_pitch \| can0:1 \| 0\.3\.1\.42 \| 600 \(0\.030 s\) \|/);
    assert.match(rendered, /\| right_lower_arm_yaw \| can0:5 \| 0\.0\.3\.32 \| 0 \(off\) \|/);
    assert.ok(rendered.endsWith(raw), "raw output kept");
    const skipped = "bin/motor-repl protocol-inspect skipped: marengo-pi (pid 42) owns CAN";
    assert.equal(renderProtocolInspection(skipped), skipped);
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
