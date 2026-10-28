import { loadConfig } from "../config.js";
import { harnessStepFailure } from "../harness/index.js";
import { execRemote } from "../ssh.js";

/** Runs the harness step check on bench-latest.log and names the check that failed. */
const cfg = loadConfig();
const full = await execRemote(cfg, "cat /opt/marengo/var/log/bench-latest.log", {
  timeoutMs: 30_000,
});
const failure = harnessStepFailure(full.stdout);
console.log("stepOk", failure === null);
if (failure !== null) console.log("fail:", failure);
console.log("failed count", (full.stdout.match(/failed/gi) ?? []).length);
console.log("fault non-zero", (full.stdout.match(/fault=0x[0-9a-fA-F]*[1-9a-fA-F]/g) ?? []));
