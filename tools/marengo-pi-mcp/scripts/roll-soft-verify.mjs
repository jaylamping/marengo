import { loadConfig } from "../dist/config.js";
import { execRemote, formatRemoteResult } from "../dist/ssh.js";
import { runRollSoftVerify } from "../dist/harness/index.js";

const cfg = loadConfig();
const out = await runRollSoftVerify(
  cfg,
  async (body, timeoutMs) =>
    formatRemoteResult(await execRemote(cfg, body, { timeoutMs: timeoutMs ?? 150_000 })),
  { config_dir: "arm_3dof_right" },
);

const logMatch = out.match(/log=([^\s]+)/);
console.log("log", logMatch?.[1] ?? "(unknown)");
console.log(out.slice(-1200));
