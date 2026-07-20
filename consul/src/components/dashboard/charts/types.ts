export type ChartTimeRange = 'session' | '5m' | '1m';

export type JointTrackingPoint = {
  time: string;
  /** Joint position (rad) — primary series for overview charts. */
  measured: number;
  /** Omitted in live Chappe mode until proto exposes commanded position. */
  commanded?: number;
  velocity?: number;
  torque?: number;
  temperature?: number;
};

type JointLimits = {
  lower: number;
  upper: number;
  effort: number;
  velocity: number;
};

type JointSafety = {
  softLower: number;
  softUpper: number;
  kPosition: number;
  kVelocity: number;
};

export type { JointLimits, JointSafety };

export type JointTrackingSeries = {
  jointName: string;
  title: string;
  description: string;
  descriptionShort: string;
  points: JointTrackingPoint[];
  limits?: JointLimits | null;
  safety?: JointSafety | null;
};
