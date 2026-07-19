import { loadConfig } from "../dist/config.js";
import { execRemote } from "../dist/ssh.js";

function defaultStepOk(out) {
  if (out.includes("[exit ") && !out.match(/\[exit 0\]/)) {
    console.log("fail: exit");
    return false;
  }
  if (out.toLowerCase().includes("failed")) {
    console.log("fail: failed substring");
    return false;
  }
  if (/fault=0x[0-9a-fA-F]*[1-9a-fA-F]/.test(out)) {
    console.log("fail: fault");
    return false;
  }
  if (/watchdog|outside \[/i.test(out)) {
    console.log("fail: watchdog/outside");
    return false;
  }
  return true;
}

const cfg = loadConfig();
const full = await execRemote(cfg, "cat /opt/marengo/var/log/bench-latest.log", {
  timeoutMs: 30_000,
});
console.log("stepOk", defaultStepOk(full.stdout));
console.log("failed count", (full.stdout.match(/failed/gi) ?? []).length);
console.log("fault non-zero", (full.stdout.match(/fault=0x[0-9a-fA-F]*[1-9a-fA-F]/g) ?? []));
