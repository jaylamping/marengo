import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { cleanTreeSchema } from "../src/tools/clean-tree.js";

describe("pi_clean_tree", () => {
  it("requires confirm: true", () => {
    assert.throws(() => cleanTreeSchema.parse({ confirm: false, mode: "stash" }));
    assert.throws(() => cleanTreeSchema.parse({ mode: "stash" }));
  });

  it("accepts all modes", () => {
    for (const mode of ["stash", "reset-hard", "clean-untracked"] as const) {
      const parsed = cleanTreeSchema.parse({ confirm: true, mode });
      assert.equal(parsed.mode, mode);
      assert.equal(parsed.confirm, true);
    }
  });

  it("defaults to stash", () => {
    const parsed = cleanTreeSchema.parse({ confirm: true });
    assert.equal(parsed.mode, "stash");
  });
});
