import { loadConfig } from "../dist/config.js";
import { execRemote, formatRemoteResult } from "../dist/ssh.js";
import { runBenchHarness } from "../dist/harness/index.js";

const cfg = loadConfig();
let rollOut = "";
const runRemote = async (body, timeoutMs) => {
  const formatted = formatRemoteResult(
    await execRemote(cfg, body, { timeoutMs: timeoutMs ?? 120_000 }),
  );
  if (body.includes("roll_sign_probe")) {
    rollOut = formatted;
  }
  return formatted;
};

function defaultStepOk(out) {
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
  confirm: true,
  confirm_weighted_motion: true,
  profile: "roll_attached",
  config_dir: "arm_2dof_right",
  skip_set_zero: true,
});

console.log("roll stepOk", defaultStepOk(rollOut));
console.log("tail", rollOut.slice(-800));
