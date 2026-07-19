import { isChappeLive } from '@/lib/chappe-config';

/** Overview honesty: never present synthetic data as live/healthy. */
export type StatusTone = 'healthy' | 'warning' | 'muted';

export type DemoBadge = {
  label: string;
  tone: StatusTone;
};

export function isWireframeMode(): boolean {
  return !isChappeLive();
}

/** Badge when metrics are layout fixtures, not machine truth. */
export function demoBadge(label: string = 'demo'): DemoBadge {
  return { label, tone: 'muted' };
}

export type JointChartFeedKind = 'live' | 'demo' | 'waiting';

export function resolveJointChartFeed(opts: {
  live: boolean;
  connected: boolean;
  pointCount: number;
}): JointChartFeedKind {
  if (!opts.live) {
    return 'demo';
  }
  if (!opts.connected || opts.pointCount === 0) {
    return 'waiting';
  }
  return 'live';
}

export function jointChartCopy(
  jointName: string | undefined,
  kind: JointChartFeedKind,
  jointType?: string
): { title: string; description: string; descriptionShort: string } {
  const name = jointName ?? 'Joint Tracking';
  switch (kind) {
    case 'live':
      return {
        title: `${name} (live)`,
        description: jointType
          ? `Measured position from Chappe robot/state. Type: ${jointType}.`
          : 'Live · Chappe',
        descriptionShort: 'Live · Chappe',
      };
    case 'demo':
      return {
        title: `${name} (demo)`,
        description: 'Synthetic series for layout — not measured from Chappe.',
        descriptionShort: 'Demo · wireframe',
      };
    case 'waiting':
      return {
        title: `${name} (waiting)`,
        description: 'Chappe connected — waiting for joint samples.',
        descriptionShort: 'Waiting · Chappe',
      };
    default: {
      const _exhaustive: never = kind;
      return _exhaustive;
    }
  }
}
