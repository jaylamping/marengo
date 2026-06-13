import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { MarengoPiConfig } from "../src/config.js";
import { registerMotionTools } from "../src/tools/motion.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config/bringup/shoulder_pitch_right_only",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

describe("marengo-pi script tool", () => {
  it("keeps bench logs current when script exits nonzero", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_marengo_pi_script.handler({
      confirm: true,
      joint: "right_shoulder_pitch",
      script: ["home", "quit"],
      timeout_sec: 10,
    });

    assert.match(script, /\} 2>&1 \| tee -a "\$LOG"/);
    assert.match(script, /PIPE_STATUS=\$\{PIPESTATUS\[0\]\}/);
    assert.match(script, /ln -sf "\$LOG" "\$LOGDIR\/bench-latest\.log"/);
    assert.match(script, /exit "\$PIPE_STATUS"/);
  });

  it("sends disable and quit after a timed hold dwell", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_hold_on.handler({
      confirm: true,
      joint: "right_shoulder_pitch",
      set_zero: false,
      position_rad: 0,
      timeout_sec: 10,
    });

    assert.match(script, /printf '%s\\n' "hold-at 0";/);
    assert.match(script, /sleep 10;/);
    assert.match(script, /printf '%s\\n' "disable";/);
    assert.match(script, /printf '%s\\n' "quit";/);
    assert.match(script, /\} \| timeout 20 \$PI_BIN/);
    assert.match(script, /bin\/motor-repl disable/);
  });

  it("expands profile-like config_dir overrides before a hold test", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_hold_on.handler({
      confirm: true,
      config_dir: "shoulder_pitch_right_only",
      joint: "right_shoulder_pitch",
      set_zero: false,
      position_rad: 0.1,
      timeout_sec: 10,
    });

    assert.match(
      script,
      /export MARENGO_CONFIG_DIR='\/opt\/marengo\/config\/bringup\/shoulder_pitch_right_only'/,
    );
    assert.doesNotMatch(
      script,
      /export MARENGO_CONFIG_DIR='shoulder_pitch_right_only'/,
    );
  });

  it("expands profile-like config_dir overrides for homing status", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_homing_status.handler({
      config_dir: "shoulder_pitch_right_only",
    });

    assert.match(
      script,
      /export MARENGO_CONFIG_DIR='\/opt\/marengo\/config\/bringup\/shoulder_pitch_right_only'/,
    );
    assert.doesNotMatch(
      script,
      /export MARENGO_CONFIG_DIR='shoulder_pitch_right_only'/,
    );
  });

  it("treats sleep N script lines as shell dwell between marengo-pi commands", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_marengo_pi_script.handler({
      confirm: true,
      joint: "right_shoulder_pitch",
      script: ["home", "enable bench", "hold-at 1.570796", "sleep 35", "hold-at 0"],
      timeout_sec: 80,
    });

    assert.match(script, /printf '%s\\n' "hold-at 1\.570796";/);
    assert.match(script, /sleep 35;/);
    assert.match(script, /printf '%s\\n' "hold-at 0";/);
    assert.match(script, /\} \| timeout 80 \$PI_BIN/);
  });

  it("pipes every script line into marengo-pi", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_marengo_pi_script.handler({
      confirm: true,
      joint: "right_shoulder_pitch",
      script: ["home", "enable bench", "status"],
      timeout_sec: 10,
    });

    assert.match(
      script,
      /\{\nprintf '%s\\n' "home";\nprintf '%s\\n' "enable bench";\nprintf '%s\\n' "status";\n\} \| timeout 10 \$PI_BIN/,
    );
  });

  it("does not select a missing fallback when help probe times out", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return body;
      },
      () => {},
    );

    await tools.pi_marengo_pi_script.handler({
      confirm: true,
      joint: "right_shoulder_pitch",
      script: ["home", "disable"],
      timeout_sec: 10,
    });

    assert.match(script, /PI_BIN=bin\/marengo-pi/);
    assert.match(script, /PI_FALLBACK="\$HOME\/marengo\/target\/release\/marengo-pi"/);
    assert.match(script, /timeout 2 "\$PI_BIN" 2>&1 \|\| true/);
    assert.match(script, /test -x "\$PI_FALLBACK"/);
  });
});
