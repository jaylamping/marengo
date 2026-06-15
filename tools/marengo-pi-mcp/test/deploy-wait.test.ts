import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { deployReadyCheckScript } from "../src/tools/deploy-wait.js";

describe("deploy wait", () => {
  it("ready check script matches rev prefix and gateway", () => {
    const script = deployReadyCheckScript("679b124");
    assert.match(script, /case "\$REV" in '679b124'\*\)/);
    assert.match(script, /marengo-gateway/);
    assert.match(script, /8080\/health/);
  });
});
