import { memo, type CSSProperties, type KeyboardEvent } from 'react';

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
  /**
   * When true (default) the row is absolutely positioned for virtualized lists.
   * Field badges are omitted in this mode — use the detail sheet instead.
   */
  positioned?: boolean;
  selected?: boolean;
  onSelect?: (entry: LogEntry) => void;
};

export const LogRow = memo(function LogRow({
  entry,
  style,
  positioned = true,
  selected = false,
  onSelect,
}: LogRowProps) {
  const fields = parseLogFields(entry.fieldsJson);
  const highlightKeys = highlightLogFieldKeys(fields);
  const otherKeys = Object.keys(fields).filter(
    (key) => !highlightKeys.includes(key as (typeof highlightKeys)[number]) && key !== '_truncated',
  );
  const hasFields = highlightKeys.length > 0 || otherKeys.length > 0;
  const showFieldBadges = !positioned && hasFields;

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (!onSelect) {
      return;
    }
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onSelect(entry);
    }
  }

  return (
    <div
      style={style}
      role={onSelect ? 'button' : undefined}
      tabIndex={onSelect ? 0 : undefined}
      onClick={onSelect ? () => onSelect(entry) : undefined}
      onKeyDown={handleKeyDown}
      className={cn(
        'w-full border-b px-3 font-mono text-xs [content-visibility:auto]',
        positioned && 'absolute left-0 top-0',
        'grid items-center',
        LOG_TABLE_GRID_CLASS,
        onSelect && 'cursor-pointer hover:bg-muted/30 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset',
        !onSelect && 'hover:bg-muted/30',
        selected && 'bg-muted/50',
        positioned ? 'h-8 min-h-8 py-0' : showFieldBadges ? 'py-1' : 'py-0',
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
      <span className={cn('min-w-0 truncate', LOG_LEVEL_STYLES[entry.level])}>
        {entry.message}
        {positioned && hasFields ? (
          <span className="ml-1 text-muted-foreground" aria-hidden>
            …
          </span>
        ) : null}
      </span>
      {showFieldBadges ? (
        <div className="col-span-4 mt-1 flex flex-wrap gap-1">
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
            <Badge variant="outline" className="px-1.5 py-0 text-[10px] font-normal text-warning">
              truncated
            </Badge>
          ) : null}
        </div>
      ) : null}
    </div>
  );
});

export const LOG_ROW_ESTIMATE_SIZE = LOG_ROW_HEIGHT_PX;
