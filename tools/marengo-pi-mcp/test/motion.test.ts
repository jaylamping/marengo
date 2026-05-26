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
