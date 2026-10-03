/** Test-only `motor-repl gravity-preview` stdout for the right 5-DOF master (robot.yaml order). */

const MASTER_JOINTS = [
  "right_shoulder_pitch",
  "right_shoulder_roll",
  "right_upper_arm_yaw",
  "right_elbow_pitch",
  "right_lower_arm_yaw",
];

/** Every master joint prints `tau_g`; joints missing from `tauG` read 0. */
export function gravityPreviewReply(tauG: Record<string, number> = {}): string {
  return MASTER_JOINTS.map((j) => `${j}: tau_g = ${(tauG[j] ?? 0).toFixed(4)} Nm`).join("\n");
}

/** Remote body of the gravity gate's preview step. */
export function isGravityPreviewBody(body: string): boolean {
  return body.includes("bin/motor-repl gravity-preview");
}
