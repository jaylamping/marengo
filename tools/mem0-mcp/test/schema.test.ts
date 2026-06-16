import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  parseNamespace,
  rejectSecrets,
  validateTopicKey,
} from "../src/schema.js";

describe("validateTopicKey", () => {
  it("accepts valid marengo namespaces", () => {
    assert.equal(validateTopicKey("sdd/pilot-change/explore"), null);
    assert.equal(validateTopicKey("feasibility/pilot-change/brief"), null);
    assert.equal(validateTopicKey("research/robotics/can-bus"), null);
    assert.equal(validateTopicKey("maintenance/prune/2026-06-16"), null);
  });

  it("rejects invalid keys", () => {
    assert.match(validateTopicKey("bad/key") ?? "", /topic_key must match/);
    assert.match(validateTopicKey("sdd/UPPER/explore") ?? "", /topic_key must match/);
  });
});

describe("rejectSecrets", () => {
  it("blocks api keys in content", () => {
    assert.match(
      rejectSecrets("token m0sk_abcdefghijklmnopqrstuvwxyz123456") ?? "",
      /secret/,
    );
  });

  it("allows normal technical content", () => {
    assert.equal(rejectSecrets("shoulder pitch hold uses Davout gate"), null);
  });
});

describe("parseNamespace", () => {
  it("maps topic prefixes", () => {
    assert.equal(parseNamespace("sdd/foo/explore"), "sdd");
    assert.equal(parseNamespace(undefined), "other");
  });
});
