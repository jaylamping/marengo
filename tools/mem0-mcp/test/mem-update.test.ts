import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { memUpdateSchema, rejectSecrets } from "../src/schema.js";

describe("memUpdateSchema", () => {
  it("accepts id and content", () => {
    const parsed = memUpdateSchema.parse({
      id: "obs-123",
      content: "## Tasks\n- [x] done",
    });
    assert.equal(parsed.id, "obs-123");
    assert.match(parsed.content, /\[x\] done/);
  });

  it("rejects empty content", () => {
    assert.throws(() => memUpdateSchema.parse({ id: "obs-123", content: "" }));
  });
});

describe("mem_update secret guard", () => {
  it("blocks api keys in update content", () => {
    assert.match(
      rejectSecrets("token m0sk_abcdefghijklmnopqrstuvwxyz123456") ?? "",
      /secret/,
    );
  });
});
