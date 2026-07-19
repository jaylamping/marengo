import { loadConfig } from "../dist/config.js";
import { execRemote, formatRemoteResult } from "../dist/ssh.js";
import { runRollDegreeSweep } from "../dist/harness/index.js";

const cfg = loadConfig();
const out = await runRollDegreeSweep(
  cfg,
  async (body, timeoutMs) =>
    formatRemoteResult(await execRemote(cfg, body, { timeoutMs: timeoutMs ?? 480_000 })),
  { config_dir: "arm_3dof_right" },
);

const logMatch = out.match(/log=([^\s]+)/);
console.log("log", logMatch?.[1] ?? "(unknown)");
console.log(out.slice(-1500));
