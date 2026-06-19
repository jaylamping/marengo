import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { loadConfig } from "../src/config.js";
import {
  MEM0_API_URL,
  MEM0_DASHBOARD_URL,
  MEM0_USER_ID,
  resolveMem0ApiUrl,
} from "../src/defaults.js";

describe("mem0 defaults", () => {
  it("uses canonical joey-pc Tailscale API URL", () => {
    assert.equal(MEM0_API_URL, "https://joey-pc.tail0b414.ts.net:8888");
    assert.equal(MEM0_DASHBOARD_URL, "https://joey-pc.tail0b414.ts.net");
    assert.equal(MEM0_USER_ID, "marengo-joey");
  });

  it("resolveMem0ApiUrl falls back to default when env unset", () => {
    const prev = process.env.MEM0_API_URL;
    delete process.env.MEM0_API_URL;
    try {
      assert.equal(resolveMem0ApiUrl(), MEM0_API_URL);
    } finally {
      if (prev === undefined) {
        delete process.env.MEM0_API_URL;
      } else {
        process.env.MEM0_API_URL = prev;
      }
    }
  });

  it("resolveMem0ApiUrl strips trailing slash", () => {
    assert.equal(
      resolveMem0ApiUrl("https://joey-pc.tail0b414.ts.net:8888/"),
      MEM0_API_URL,
    );
  });
});

describe("loadConfig", () => {
  it("defaults apiUrl and userId when only MEM0_API_KEY is set", () => {
    const prevUrl = process.env.MEM0_API_URL;
    const prevKey = process.env.MEM0_API_KEY;
    const prevUser = process.env.MEM0_USER_ID;
    delete process.env.MEM0_API_URL;
    delete process.env.MEM0_USER_ID;
    process.env.MEM0_API_KEY = "m0sk_test_key_for_unit_test_only";
    try {
      const cfg = loadConfig();
      assert.equal(cfg.apiUrl, MEM0_API_URL);
      assert.equal(cfg.userId, MEM0_USER_ID);
    } finally {
      if (prevUrl === undefined) {
        delete process.env.MEM0_API_URL;
      } else {
        process.env.MEM0_API_URL = prevUrl;
      }
      if (prevKey === undefined) {
        delete process.env.MEM0_API_KEY;
      } else {
        process.env.MEM0_API_KEY = prevKey;
      }
      if (prevUser === undefined) {
        delete process.env.MEM0_USER_ID;
      } else {
        process.env.MEM0_USER_ID = prevUser;
      }
    }
  });
});
