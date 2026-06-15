import { memo, type CSSProperties } from 'react';

import type { LogEntry } from '@/data/logs';
import {
  LOG_LEVEL_STYLES,
  LOG_ROW_HEIGHT_PX,
  LOG_TABLE_GRID_CLASS,
} from '@/components/dashboard/logs/constants';
import {
  formatLogTimestamp,
  highlightLogFieldKeys,
  parseLogFields,
} from '@/components/dashboard/logs/utils';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

type LogRowProps = {
  entry: LogEntry;
  style?: CSSProperties;
};

export const LogRow = memo(function LogRow({ entry, style }: LogRowProps) {
  const fields = parseLogFields(entry.fieldsJson);
  const highlightKeys = highlightLogFieldKeys(fields);
  const otherKeys = Object.keys(fields).filter(
    (key) => !highlightKeys.includes(key as (typeof highlightKeys)[number]) && key !== '_truncated',
  );
  const hasFields = highlightKeys.length > 0 || otherKeys.length > 0;

  return (
    <div
      style={style}
      className={cn(
        'absolute left-0 top-0 w-full border-b px-3 font-mono text-xs hover:bg-muted/30 [content-visibility:auto]',
        hasFields ? 'py-1' : 'grid items-center',
        !hasFields && LOG_TABLE_GRID_CLASS,
      )}
    >
      <div className={cn(hasFields && LOG_TABLE_GRID_CLASS, 'items-center')}>
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
      {hasFields ? (
        <div className="mt-1 flex flex-wrap gap-1 pl-[calc(180px+88px+120px+12px)]">
          {highlightKeys.map((key) => (
            <Badge key={key} variant="secondary" className="px-1.5 py-0 text-[10px] font-normal">
              {key}={fields[key]}
            </Badge>
          ))}
          {otherKeys.slice(0, 6).map((key) => (
            <Badge key={key} variant="outline" className="px-1.5 py-0 text-[10px] font-normal text-muted-foreground">
              {key}={fields[key]}
            </Badge>
          ))}
          {fields._truncated === 'true' ? (
            <Badge variant="outline" className="px-1.5 py-0 text-[10px] font-normal text-amber-500">
              truncated
            </Badge>
          ) : null}
        </div>
      ) : null}
    </div>
  );
});

export const LOG_ROW_ESTIMATE_SIZE = LOG_ROW_HEIGHT_PX;
