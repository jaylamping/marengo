import type {
  ChartTimeRange,
  JointTrackingPoint,
} from '@/components/dashboard/charts/types';

export function filterTrackingPointsByTimeRange(
  points: JointTrackingPoint[],
  timeRange: ChartTimeRange,
): JointTrackingPoint[] {
  return points.filter((point) => {
    const [, seconds] = point.time.split(':').map(Number);

    if (timeRange === 'session') {
      return true;
    }

    if (timeRange === '5m') {
      return seconds <= 55 || point.time.startsWith('00:');
    }

    return seconds >= 55 || point.time.startsWith('01:') || point.time === '02:00';
  });
}
