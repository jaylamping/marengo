export type ChartTimeRange = 'session' | '5m' | '1m';

export type JointTrackingPoint = {
  time: string;
  commanded: number;
  measured: number;
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
