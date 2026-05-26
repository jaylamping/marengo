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
