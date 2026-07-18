import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { deployRevLogCommand } from "../src/tools/deploy.js";

describe("deploy rev log", () => {
  it("reads canonical /opt/marengo/.deploy-rev", () => {
    const cmd = deployRevLogCommand("/opt/marengo");
    assert.match(cmd, /cat '\/opt\/marengo\/\.deploy-rev'/);
    assert.match(cmd, /\(no \.deploy-rev\)/);
  });

  it("quotes custom install roots", () => {
    const cmd = deployRevLogCommand("/opt/marengo-test");
    assert.match(cmd, /\/opt\/marengo-test\/\.deploy-rev/);
  });
});
