import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { MarengoPiConfig } from "../src/config.js";
import { remotePreamble, wrapRemote, wrapRemoteWithConfig, wrapStagingRemote } from "../src/env.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

// WP-F: every MCP-started marengo-pi is a stdin-scripted session and must claim
// motion ownership, so a Consul tab (Chappe) can only disable/stop it.
describe("MCP motion ownership claim", () => {
  it("the remote preamble exports MARENGO_MOTION_OWNER=stdin", () => {
    assert.match(remotePreamble(cfg), /^export MARENGO_MOTION_OWNER=stdin$/m);
  });

  it("every wrapper that launches marengo-pi inherits the claim", () => {
    for (const script of [
      wrapRemote(cfg, "true"),
      wrapRemoteWithConfig(cfg, "true"),
      wrapStagingRemote(cfg, "true"),
    ]) {
      assert.match(script, /export MARENGO_MOTION_OWNER=stdin/);
    }
  });

  it("the claim is set before the body runs", () => {
    const script = wrapRemote(cfg, "echo body");
    assert.ok(script.indexOf("MARENGO_MOTION_OWNER=stdin") < script.indexOf("echo body"));
  });
});
