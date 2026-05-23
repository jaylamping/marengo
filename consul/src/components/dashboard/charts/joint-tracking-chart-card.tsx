import { useMemo } from 'react';

import { ChartTimeRangeControls } from '@/components/dashboard/charts/chart-time-range-controls';
import { dummyShoulderPitchTracking } from '@/components/dashboard/charts/constants';
import { useChartTimeRange } from '@/components/dashboard/charts/hooks/use-chart-time-range';
import { JointTrackingAreaChart } from '@/components/dashboard/charts/joint-tracking-area-chart';
import type { JointTrackingSeries } from '@/components/dashboard/charts/types';
import { filterTrackingPointsByTimeRange } from '@/components/dashboard/charts/utils';
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';

type JointTrackingChartCardProps = {
  series?: JointTrackingSeries;
};

export function JointTrackingChartCard({
  series = dummyShoulderPitchTracking,
}: JointTrackingChartCardProps) {
  const { timeRange, setTimeRange } = useChartTimeRange();
  const filteredData = useMemo(
    () => filterTrackingPointsByTimeRange(series.points, timeRange),
    [series.points, timeRange],
  );

  return (
    <Card className="@container/card">
      <CardHeader>
        <CardTitle>{series.title}</CardTitle>
        <CardDescription>
          <span className="hidden @[540px]/card:block">{series.description}</span>
          <span className="@[540px]/card:hidden">{series.descriptionShort}</span>
        </CardDescription>
        <CardAction>
          <ChartTimeRangeControls
            timeRange={timeRange}
            onTimeRangeChange={setTimeRange}
          />
        </CardAction>
      </CardHeader>
      <CardContent className="px-2 pt-4 sm:px-6 sm:pt-6">
        <JointTrackingAreaChart data={filteredData} />
      </CardContent>
    </Card>
  );
}
