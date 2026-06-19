import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  MEMORY_TOOL_NAMES,
  countRawLogLines,
  memGetByTopicKeySchema,
  memSaveSchema,
  normalizeContentInput,
  parseNamespace,
  rejectOversizedContent,
  rejectRawLogs,
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

describe("memGetByTopicKeySchema", () => {
  it("accepts topic_key", () => {
    const parsed = memGetByTopicKeySchema.parse({
      topic_key: "sdd/foo/tasks",
      project: "marengo",
    });
    assert.equal(parsed.topic_key, "sdd/foo/tasks");
  });

  it("accepts query as legacy alias for topic_key", () => {
    const parsed = memGetByTopicKeySchema.parse({
      query: "maintenance/session-handoff/marengo",
      project: "marengo",
    });
    assert.equal(parsed.topic_key, "maintenance/session-handoff/marengo");
  });

  it("rejects missing topic_key and query", () => {
    assert.throws(() => memGetByTopicKeySchema.parse({ project: "marengo" }));
  });
});

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

  it("allows distilled docs that mention candump or journalctl tools", () => {
    const proposal = `# Proposal: consul-actuator-harness

## Intent
Bridge sim-to-real by adding /actuators page to Consul.

## Test plan
- After motion: pi_candump_summary plus position trace
- Check journalctl -u marengo-pi for faults
`;
    assert.equal(validateMemoryContent(proposal), null);
  });

  it("blocks pasted candump log dumps", () => {
    const dump = [
      "(0.001234) can0 701#AABBCCDD",
      "(0.002345) can0 701#BBCCDDEE",
      "(0.003456) can0 701#CCDDEEFF",
    ].join("\n");
    assert.equal(countRawLogLines(dump), 3);
    assert.match(rejectRawLogs(dump) ?? "", /raw log output/);
  });

  it("allows a single example candump line in docs", () => {
    const doc = "Example frame: (0.001234) can0 701#AABBCCDD";
    assert.equal(rejectRawLogs(doc), null);
  });

  it("coerces Anthropic content block arrays to plain strings", () => {
    const parsed = memSaveSchema.parse({
      title: "sdd/foo/proposal",
      topic_key: "sdd/foo/proposal",
      content: [{ text: "# Proposal\n\nBody text.", type: "text" }],
    });
    assert.equal(parsed.content, "# Proposal\n\nBody text.");
  });
});

describe("normalizeContentInput", () => {
  it("passes through plain strings", () => {
    assert.equal(normalizeContentInput("hello"), "hello");
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
