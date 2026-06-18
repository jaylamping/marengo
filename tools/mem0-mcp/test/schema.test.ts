import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  MEMORY_TOOL_NAMES,
  parseNamespace,
  rejectOversizedContent,
  rejectSecrets,
  validateMemoryContent,
  validateTopicKey,
} from "../src/schema.js";
import { registerMemoryTools } from "../src/tools/memory.js";
import {
  isProtected,
  sortDeletableForPrune,
  topicKeyFromRow,
} from "../src/prune-policy.js";

describe("validateTopicKey", () => {
  it("accepts valid marengo namespaces", () => {
    assert.equal(validateTopicKey("sdd/pilot-change/explore"), null);
    assert.equal(validateTopicKey("sdd/init/marengo"), null);
    assert.equal(validateTopicKey("feasibility/pilot-change/brief"), null);
    assert.equal(validateTopicKey("research/robotics/can-bus"), null);
    assert.equal(validateTopicKey("maintenance/prune/2026-06-16"), null);
    assert.equal(validateTopicKey("maintenance/skill-registry"), null);
    assert.equal(validateTopicKey("decision/control/davout-gate"), null);
    assert.equal(validateTopicKey("hardware/shoulder/load-limits"), null);
    assert.equal(validateTopicKey("cad/torso-frame/mate-origin"), null);
    assert.equal(validateTopicKey("pi/can/interface-up"), null);
    assert.equal(validateTopicKey("control/gravity/shoulder-pitch"), null);
    assert.equal(validateTopicKey("software/robstride/socketcan-tests"), null);
  });

  it("rejects invalid keys", () => {
    assert.match(validateTopicKey("bad/key") ?? "", /topic_key must match/);
    assert.match(validateTopicKey("sdd/UPPER/explore") ?? "", /topic_key must match/);
  });
});

describe("validateMemoryContent", () => {
  it("blocks api keys in content", () => {
    assert.match(
      validateMemoryContent("token m0sk_abcdefghijklmnopqrstuvwxyz123456") ?? "",
      /secret/,
    );
  });

  it("blocks oversized content", () => {
    assert.match(rejectOversizedContent("x".repeat(33_000)) ?? "", /max length/);
  });

  it("allows normal technical content", () => {
    assert.equal(
      validateMemoryContent("shoulder pitch hold uses Davout gate"),
      null,
    );
  });
});

describe("parseNamespace", () => {
  it("maps topic prefixes including expanded namespaces", () => {
    assert.equal(parseNamespace("sdd/foo/explore"), "sdd");
    assert.equal(parseNamespace("pi/can/interface-up"), "pi");
    assert.equal(parseNamespace(undefined), "other");
  });
});

describe("registerMemoryTools", () => {
  it("registers the expected tool names", () => {
    const tools = registerMemoryTools({
      apiUrl: "http://example.test",
      apiKey: "test-key",
      userId: "marengo-joey",
    });
    assert.deepEqual(Object.keys(tools).sort(), [...MEMORY_TOOL_NAMES].sort());
  });
});

describe("isProtected", () => {
  it("protects critical maintenance keys", () => {
    assert.equal(isProtected("maintenance/skill-registry"), true);
    assert.equal(isProtected("maintenance/session-handoff/marengo"), true);
  });

  it("protects expert and research namespaces", () => {
    assert.equal(isProtected("expert/robotics/can-timeout"), true);
    assert.equal(isProtected("research/robotics/can-bus"), true);
  });

  it("protects recent subsystem memories", () => {
    const recent = new Date().toISOString();
    assert.equal(isProtected("pi/can/interface-up", recent), true);
    assert.equal(isProtected("cad/torso-frame/mate-origin", recent), true);
  });

  it("allows stale subsystem memories to be pruned", () => {
    const stale = new Date(Date.now() - 40 * 24 * 60 * 60 * 1000).toISOString();
    assert.equal(isProtected("pi/can/interface-up", stale), false);
  });
});

describe("sortDeletableForPrune", () => {
  it("orders oldest memories first", () => {
    const sorted = sortDeletableForPrune([
      { created_at: "2026-06-10T00:00:00.000Z" },
      { created_at: "2026-06-01T00:00:00.000Z" },
      { created_at: "2026-06-15T00:00:00.000Z" },
    ]);
    assert.equal(sorted[0].created_at, "2026-06-01T00:00:00.000Z");
    assert.equal(sorted[2].created_at, "2026-06-15T00:00:00.000Z");
  });
});

describe("topicKeyFromRow", () => {
  it("reads metadata topic_key", () => {
    assert.equal(
      topicKeyFromRow({ metadata: { topic_key: "software/consul/memory-ui" } }),
      "software/consul/memory-ui",
    );
  });
});
