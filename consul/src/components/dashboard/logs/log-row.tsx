import { memo, type CSSProperties } from 'react';

import type { LogEntry } from '@/data/logs';
import {
  LOG_LEVEL_STYLES,
  LOG_ROW_HEIGHT_PX,
  LOG_TABLE_GRID_CLASS,
} from '@/components/dashboard/logs/constants';
import { formatLogTimestamp } from '@/components/dashboard/logs/utils';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

type LogRowProps = {
  entry: LogEntry;
  style?: CSSProperties;
};

export const LogRow = memo(function LogRow({ entry, style }: LogRowProps) {
  return (
    <div
      style={style}
      className={cn(
        LOG_TABLE_GRID_CLASS,
        'absolute left-0 top-0 w-full items-center border-b px-3 font-mono text-xs hover:bg-muted/30 [content-visibility:auto]',
      )}
    >
      <span className="truncate tabular-nums text-muted-foreground">
        {formatLogTimestamp(entry.timestamp)}
      </span>
      <Badge
        variant="outline"
        className={cn('w-fit px-1.5 py-0 text-[10px] uppercase', LOG_LEVEL_STYLES[entry.level])}
      >
        {entry.level}
      </Badge>
      <span className="truncate text-muted-foreground">{entry.source}</span>
      <span className={cn('truncate', LOG_LEVEL_STYLES[entry.level])}>
        {entry.message}
      </span>
    </div>
  );
});

export const LOG_ROW_ESTIMATE_SIZE = LOG_ROW_HEIGHT_PX;
