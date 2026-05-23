import { CHART_TIME_RANGE_OPTIONS } from '@/components/dashboard/charts/constants';
import type { ChartTimeRange } from '@/components/dashboard/charts/types';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  ToggleGroup,
  ToggleGroupItem,
} from '@/components/ui/toggle-group';

type ChartTimeRangeControlsProps = {
  timeRange: ChartTimeRange;
  onTimeRangeChange: (timeRange: ChartTimeRange) => void;
};

export function ChartTimeRangeControls({
  timeRange,
  onTimeRangeChange,
}: ChartTimeRangeControlsProps) {
  return (
    <>
      <ToggleGroup
        multiple={false}
        value={timeRange ? [timeRange] : []}
        onValueChange={(value) => {
          onTimeRangeChange((value[0] ?? 'session') as ChartTimeRange);
        }}
        variant="outline"
        className="hidden *:data-[slot=toggle-group-item]:px-4! @[767px]/card:flex"
      >
        {CHART_TIME_RANGE_OPTIONS.map((option) => (
          <ToggleGroupItem key={option.value} value={option.value}>
            {option.label}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
      <Select
        value={timeRange}
        onValueChange={(value) => {
          if (value !== null) {
            onTimeRangeChange(value as ChartTimeRange);
          }
        }}
      >
        <SelectTrigger
          className="flex w-40 **:data-[slot=select-value]:block **:data-[slot=select-value]:truncate @[767px]/card:hidden"
          size="sm"
          aria-label="Select time range"
        >
          <SelectValue placeholder="Session" />
        </SelectTrigger>
        <SelectContent className="rounded-xl">
          {CHART_TIME_RANGE_OPTIONS.map((option) => (
            <SelectItem key={option.value} value={option.value} className="rounded-lg">
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </>
  );
}
