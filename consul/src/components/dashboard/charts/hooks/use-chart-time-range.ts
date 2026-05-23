import * as React from 'react';

import type { ChartTimeRange } from '@/components/dashboard/charts/types';
import { useIsMobile } from '@/hooks/use-mobile';

export function useChartTimeRange(defaultRange: ChartTimeRange = 'session') {
  const isMobile = useIsMobile();
  const [timeRange, setTimeRange] = React.useState<ChartTimeRange>(defaultRange);

  React.useEffect(() => {
    if (isMobile) {
      setTimeRange('1m');
    }
  }, [isMobile]);

  return { timeRange, setTimeRange };
}
