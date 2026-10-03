import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { MarengoPiConfig } from "../src/config.js";
import { registerAdminTools } from "../src/tools/admin.js";
import { runSyncTree } from "../src/tools/sync-tree.js";
import { runCleanTree } from "../src/tools/clean-tree.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

async function scriptOf(tool: "pi_build" | "pi_can_up" | "pi_git_pull"): Promise<string> {
  let script = "";
  const tools = registerAdminTools(cfg, async (body) => {
    script = body;
    return body;
  });
  await tools[tool].handler();
  return script;
}

describe("admin tools", () => {
  it("pi_build builds in staging and leaves every root step to sudo-allowed install-pi.sh", async () => {
    const script = await scriptOf("pi_build");
    assert.match(script, /cd '\/home\/joey\/marengo'\nbash \.\/scripts\/pi-native-build\.sh\nsudo -n \/home\/joey\/marengo\/scripts\/install-pi\.sh$/);
    // The deploy user has no general passwordless sudo; only the sudoers-listed scripts work.
    assert.doesNotMatch(script, /\bsudo (?!-n \/home\/joey\/marengo\/scripts\/install-pi\.sh)/);
  });

  it("pi_can_up does not bounce links while a process owns CAN", async () => {
    const script = await scriptOf("pi_can_up");
    const ownedStart = script.indexOf('if [[ -n "$CAN_OWNER" ]]; then');
    const elseAt = script.indexOf("\nelse\n", ownedStart);
    assert.ok(ownedStart >= 0 && elseAt > ownedStart);
    assert.doesNotMatch(script.slice(ownedStart, elseAt), /^sudo -n .*can-up\.sh/m);
    assert.match(script.slice(elseAt), /^sudo -n \/opt\/marengo\/scripts\/can-up\.sh can0 can1$/m);
  });

  it("runs git tools in the staging checkout, not the /opt install tree", async () => {
    const capture = async (run: (rr: (body: string) => Promise<string>) => Promise<string>) => {
      let script = "";
      await run(async (body) => {
        script = body;
        return body;
      });
      return script;
    };
    const scripts = [
      await scriptOf("pi_git_pull"),
      await capture((rr) => runSyncTree(cfg, rr)),
      await capture((rr) => runCleanTree(cfg, rr, { confirm: true, mode: "stash" })),
    ];
    for (const script of scripts) {
      // The preamble cds to /opt/marengo first; the staging cd must follow it.
      const lastCd = script.match(/^cd .*$/gm)?.at(-1);
      assert.equal(lastCd, "cd '/home/joey/marengo'");
      assert.ok(script.indexOf(lastCd) < script.indexOf("git "));
    }
  });
});
