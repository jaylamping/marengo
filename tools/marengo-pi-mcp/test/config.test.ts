import { describe, it, afterEach } from "node:test";
import assert from "node:assert/strict";
import { isBenchProfile } from "../src/bench-profiles.js";
import { loadConfig } from "../src/config.js";

describe("loadConfig / isBenchProfile", () => {
  const prev = { ...process.env };

  afterEach(() => {
    for (const key of Object.keys(process.env)) {
      if (!(key in prev)) delete process.env[key];
    }
    for (const [k, v] of Object.entries(prev)) {
      if (v === undefined) delete process.env[k];
      else process.env[k] = v;
    }
  });

  it("accepts known profiles", () => {
    assert.equal(isBenchProfile("yaw_attached"), true);
    assert.equal(isBenchProfile("roll_attached"), true);
    assert.equal(isBenchProfile("not_a_profile"), false);
  });

  it("defaults benchProfile to bare_motor", () => {
    delete process.env.MARENGO_BENCH_PROFILE;
    const cfg = loadConfig();
    assert.equal(cfg.benchProfile, "bare_motor");
  });

  it("throws on invalid MARENGO_BENCH_PROFILE", () => {
    process.env.MARENGO_BENCH_PROFILE = "yaw_typo";
    assert.throws(() => loadConfig(), /Invalid MARENGO_BENCH_PROFILE/);
  });

  it("accepts yaw_attached from env", () => {
    process.env.MARENGO_BENCH_PROFILE = "yaw_attached";
    const cfg = loadConfig();
    assert.equal(cfg.benchProfile, "yaw_attached");
  });
});
