import { useState, useMemo } from 'react';

import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import { ChartTimeRangeControls } from '@/components/dashboard/charts/chart-time-range-controls';
import { dummyShoulderPitchTracking } from '@/components/dashboard/charts/constants';
import { useChartTimeRange } from '@/components/dashboard/charts/hooks/use-chart-time-range';
import { useThrottledValue } from '@/hooks/use-throttled-value';
import { isChappeLive } from '@/lib/chappe-config';
import {
  jointChartCopy,
  resolveJointChartFeed,
} from '@/lib/telemetry-source';
import { useRobotStore } from '@/state/robotStore';
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { cn } from '@/lib/utils';
import { useRobotModel } from '@/urdf/RobotModelContext';

const CHART_RENDER_MS = 300;

type JointTrackingChartCardProps = {
  series?: JointTrackingSeries; // deprecated, will be removed when URDF is fully wired
};

export function JointTrackingChartCard({ series: _seriesProp }: JointTrackingChartCardProps) {
  const trackingPointsByJoint = useRobotStore((s) => s.trackingPointsByJoint);
  const connected = useRobotStore((s) => s.connected);

  const model = useRobotModel();
  const jointNames = Array.from(model.joints.keys());
  const defaultJoint = jointNames.find((name) => {
    const j = model.getJoint(name);
    return j?.type === 'revolute' || j?.type === 'continuous';
  });

  const [selectedJoint, setSelectedJoint] = useState<string>(defaultJoint || jointNames[0]);

  const livePoints = trackingPointsByJoint[selectedJoint] || [];
  const chartPoints = useThrottledValue(livePoints, CHART_RENDER_MS);

  const jointSpec = model.getJoint(selectedJoint);

  const series: JointTrackingSeries = useMemo(() => {
    const live = isChappeLive();
    const feed = resolveJointChartFeed({
      live,
      connected,
      pointCount: chartPoints.length,
    });
    const copy = jointChartCopy(selectedJoint, feed, jointSpec?.type);
    const points =
      feed === 'live'
        ? chartPoints
        : feed === 'demo'
          ? dummyShoulderPitchTracking.points
          : [];
    return {
      jointName: selectedJoint,
      title: copy.title,
      description: copy.description,
      descriptionShort: copy.descriptionShort,
      points,
      limits: jointSpec?.limit,
      safety: jointSpec?.safety,
    };
  }, [selectedJoint, jointSpec, chartPoints, connected]);

  const { timeRange, setTimeRange } = useChartTimeRange();
  const filteredData = useMemo(
    () => filterTrackingPointsByTimeRange(series.points, timeRange),
    [series.points, timeRange],
  );

  return (
    <Card
      variant="panel"
      className={cn('@container/card flex h-full min-h-[20rem] flex-col', dashboardPanelCardClassName)}
    >
      <CardHeader>
        <CardTitle>{series.title}</CardTitle>
        <CardDescription>
          <span className="hidden @[540px]/card:block">{series.description}</span>
          <span className="@[540px]/card:hidden">{series.descriptionShort}</span>
        </CardDescription>
        <CardAction>
          <div className="flex items-center gap-2">
            <Select value={selectedJoint} onValueChange={(val) => val !== null && setSelectedJoint(val)}>
              <SelectTrigger className="w-[160px]">
                <SelectValue placeholder="Select joint" />
              </SelectTrigger>
              <SelectContent>
                {jointNames.map((name) => (
                  <SelectItem key={name} value={name}>
                    {name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <ChartTimeRangeControls
              timeRange={timeRange}
              onTimeRangeChange={setTimeRange}
            />
          </div>
        </CardAction>
      </CardHeader>
      <CardContent className="px-2 pt-4 sm:px-6 sm:pt-6">
        <JointTrackingAreaChart
          data={filteredData}
          limits={series.limits}
          safety={series.safety}
        />
      </CardContent>
    </Card>
  );
}
