/** Test-only `motor-repl gravity-preview` stdout for the right 5-DOF master (robot.yaml order). */

import { MASTER_JOINTS } from "../src/bench-profiles.js";

/** Every master joint prints `tau_g`; joints missing from `tauG` read 0. */
export function gravityPreviewReply(tauG: Record<string, number> = {}): string {
  return MASTER_JOINTS.map((j) => `${j}: tau_g = ${(tauG[j] ?? 0).toFixed(4)} Nm`).join("\n");
}

/** Remote body of the gravity gate's preview step. */
export function isGravityPreviewBody(body: string): boolean {
  return body.includes("bin/motor-repl gravity-preview");
}

/** `CalibrationDeps.readFile` over in-memory workstation files; anything else rejects with ENOENT. */
export function localFiles(files: Record<string, string> = {}): (file: string) => Promise<string> {
  return async (file) => {
    const body = files[file];
    if (body === undefined) throw Object.assign(new Error(`ENOENT: ${file}`), { code: "ENOENT" });
    return body;
  };
}
