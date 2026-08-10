/**
 * One-shot CLI: run the roll_attached harness (includes roll_hold_sweep).
 * Formerly scripts/roll-degree-sweep.mjs — uses runBenchHarness directly.
 */
import { loadConfig } from "../config.js";
import { execRemote, formatRemoteResult } from "../ssh.js";
import { runBenchHarness } from "../harness/index.js";

const cfg = loadConfig();
const out = await runBenchHarness(
  cfg,
  async (body: string, timeoutMs?: number) =>
    formatRemoteResult(await execRemote(cfg, body, { timeoutMs: timeoutMs ?? 480_000 })),
  {
    profile: "roll_attached",
    config_dir: "arm_3dof_right",
    skip_set_zero: true,
  },
);

const logMatch = out.match(/log=([^\s]+)/);
console.log("log", logMatch?.[1] ?? "(unknown)");
console.log(out.slice(-1500));
