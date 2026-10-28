import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { chmodSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

const pkg = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/** Gate readout of a Pi whose marengo-pi runs Active with a fresh heartbeat. */
function activeGateOutput(): string {
  const now = Date.now();
  const varint = (n: number) => {
    const out: number[] = [];
    do {
      let byte = n % 128;
      n = Math.floor(n / 128);
      if (n > 0) byte |= 0x80;
      out.push(byte);
    } while (n > 0);
    return out;
  };
  const b64 = (bytes: number[]) => Buffer.from(bytes).toString("base64");
  const config = Buffer.from(JSON.stringify({ persist_ok: true, persist_pending: false })).toString("base64");
  return [
    "mgmt_marengo_pi=1",
    "mgmt_motor_repl=0",
    `mgmt_safety_b64=${b64([0x08, ...varint(now), 0x10, 3])}`,
    `mgmt_heartbeat_b64=${b64([0x08, ...varint(now)])}`,
    `mgmt_config_b64=${config}`,
    `mgmt_now_ms=${now}`,
  ].join("\n");
}

describe("MCP server result flags (end to end over stdio)", () => {
  it("a refused management call comes back with isError: true", { timeout: 60_000 }, async () => {
    const dir = mkdtempSync(path.join(tmpdir(), "mcp-is-error-"));
    const bin = path.join(dir, "bin");
    mkdirSync(bin);
    const gate = path.join(dir, "gate.txt");
    writeFileSync(gate, `${activeGateOutput()}\n`);
    // Fake ssh: every remote script gets the Active gate readout.
    writeFileSync(path.join(bin, "ssh"), `#!/bin/bash\ncat >/dev/null\ncat ${gate}\n`);
    chmodSync(path.join(bin, "ssh"), 0o755);

    const transport = new StdioClientTransport({
      command: process.execPath,
      args: ["--import", "tsx", "src/index.ts"],
      cwd: pkg,
      env: { ...(process.env as Record<string, string>), PATH: `${bin}:${process.env.PATH}`, HOME: dir },
      stderr: "ignore",
    });
    const client = new Client({ name: "is-error-test", version: "0" });
    await client.connect(transport);
    try {
      const result = await client.callTool({ name: "pi_build", arguments: {} });
      const text = (result.content as Array<{ text: string }>)[0].text;
      assert.match(text, /pi_build refused: marengo-pi is running in mode Active/);
      assert.equal(result.isError, true);
    } finally {
      await client.close();
    }
  });
});
