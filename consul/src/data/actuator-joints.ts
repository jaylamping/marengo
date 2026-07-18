/** Four bench-wired joints on the 4-DOF left arm slice (can0 ids 14–17). */
export const WIRED_BENCH_JOINTS = [
  'left_shoulder_roll',
  'left_shoulder_pitch',
  'left_upper_arm_yaw',
  'left_elbow',
] as const;

export type WiredBenchJoint = (typeof WIRED_BENCH_JOINTS)[number];

export function isWiredBenchJoint(jointName: string): jointName is WiredBenchJoint {
  return (WIRED_BENCH_JOINTS as readonly string[]).includes(jointName);
}
