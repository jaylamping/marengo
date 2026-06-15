import { memo } from 'react';

import { LOG_LEVELS } from '@/data/logs';
import { useLogActions, useLogBufferSnapshot, useLogPaused } from '@/components/dashboard/logs/hooks/use-log-controls';
import { useLogsFilter } from '@/components/dashboard/logs/logs-filter-context';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { cn } from '@/lib/utils';

function formatCount(value: number): string {
  return value.toLocaleString();
}

export const LogsToolbar = memo(function LogsToolbar({
  autoFollow,
  onAutoFollowChange,
}: {
  autoFollow?: boolean;
  onAutoFollowChange?: (next: boolean) => void;
}) {
  const paused = useLogPaused();
  const { setPaused, clear } = useLogActions();
  const stats = useLogBufferSnapshot();
  const {
    levelFilter,
    setLevelFilter,
    searchQuery,
    setSearchQuery,
    deferredSearchQuery,
  } = useLogsFilter();

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-base font-medium">Live log stream</h2>
          <p className="text-sm text-muted-foreground">
            {formatCount(stats.count)} buffered · WARN {stats.levelCounts.WARN} · ERROR{' '}
            {stats.levelCounts.ERROR}
            {levelFilter !== 'all' ? ` · filter ${levelFilter}` : ''}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button
            size="sm"
            variant={!paused ? 'default' : 'outline'}
            onClick={() => setPaused(!paused)}
          >
            {!paused ? 'Live' : 'Paused'}
          </Button>
          {onAutoFollowChange ? (
            <Button
              size="sm"
              variant={autoFollow ? 'default' : 'outline'}
              onClick={() => onAutoFollowChange(!autoFollow)}
            >
              {autoFollow ? 'Following' : 'Follow off'}
            </Button>
          ) : null}
          <Button size="sm" variant="outline" onClick={() => clear()}>
            Clear
          </Button>
        </div>
      </div>

      <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        <Tabs
          value={levelFilter}
          onValueChange={(value) => setLevelFilter(value as typeof levelFilter)}
        >
          <TabsList>
            <TabsTrigger value="all">All</TabsTrigger>
            {LOG_LEVELS.map((level) => (
              <TabsTrigger key={level} value={level}>
                {level}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>

        <Input
          value={searchQuery}
          onChange={(event) => setSearchQuery(event.target.value)}
          placeholder="Filter message, source, level…"
          className={cn(
            'w-full lg:max-w-sm',
            searchQuery !== deferredSearchQuery && 'opacity-80',
          )}
        />
      </div>
    </div>
  );
});
