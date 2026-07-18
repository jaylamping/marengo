import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { syncTreeSchema } from "../src/tools/sync-tree.js";

describe("pi_sync_tree", () => {
  it("accepts empty arguments", () => {
    const parsed = syncTreeSchema.parse({});
    assert.deepEqual(parsed, {});
  });

  it("rejects unexpected arguments", () => {
    assert.throws(() => syncTreeSchema.parse({ branch: "main" }));
  });
});
