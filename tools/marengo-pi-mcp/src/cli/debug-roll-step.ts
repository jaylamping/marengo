import { loadConfig } from "../config.js";
import { execRemote, formatRemoteResult } from "../ssh.js";
import { runBenchHarness } from "../harness/index.js";

const cfg = loadConfig();
let rollOut = "";
const runRemote = async (body: string, timeoutMs?: number): Promise<string> => {
  const formatted = formatRemoteResult(
    await execRemote(cfg, body, { timeoutMs: timeoutMs ?? 120_000 }),
  );
  if (body.includes("roll_sign_probe")) {
    rollOut = formatted;
  }
  return formatted;
};

/** Diagnostic checks for roll probe output (mirrors harness defaults, with logging). */
function debugRollStepOk(out: string): boolean {
  const exitMatch = out.match(/\[exit (\d+)\]/);
  const checks = {
    exit:
      exitMatch !== null &&
      Number(exitMatch[1]) !== 0 &&
      Number(exitMatch[1]) !== 2,
    failed: out.toLowerCase().includes("failed"),
    fault: /fault=0x[0-9a-fA-F]*[1-9a-fA-F]/.test(out),
    wd: /watchdog|outside \[/i.test(out),
  };
  console.log("checks", checks);
  return !Object.values(checks).some(Boolean);
}

await runBenchHarness(cfg, runRemote, {
  profile: "roll_attached",
  config_dir: "arm_3dof_right",
  skip_set_zero: true,
});

console.log("roll stepOk", debugRollStepOk(rollOut));
console.log("tail", rollOut.slice(-800));
