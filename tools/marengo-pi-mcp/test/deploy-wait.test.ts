import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { deployReadyCheckScript } from "../src/tools/deploy-wait.js";

describe("deploy wait", () => {
  it("ready check requires the gateway health contract and configured listeners", () => {
    const script = deployReadyCheckScript("679b124");
    assert.match(script, /case "\$REV" in '679b124'\*\)/);
    assert.match(script, /marengo-gateway/);
    assert.match(script, /8080\/health/);
    assert.match(script, /h\.get\("ok"\) is True/);
    assert.match(script, /get\("https_required"\)/);
  });
});
