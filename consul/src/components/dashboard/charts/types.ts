export type ChartTimeRange = 'session' | '5m' | '1m';

export type JointTrackingPoint = {
  time: string;
  commanded: number;
  measured: number;
};

export type JointTrackingSeries = {
  jointName: string;
  title: string;
  description: string;
  descriptionShort: string;
  points: JointTrackingPoint[];
};
