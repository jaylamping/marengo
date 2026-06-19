import { useCallback, useState } from 'react';

import { logsTabsVariant } from '@/components/dashboard/logs/constants';
import { LogRow } from '@/components/dashboard/logs/log-row';
import type { LogEntry, LogLevel } from '@/data/logs';
import { fetchStructuredLogs } from '@/lib/log-api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';

const SOURCE_PRESETS = [
  { label: 'All', target: '' },
  { label: 'davout', target: 'davout' },
  { label: 'berthier', target: 'berthier' },
  { label: 'robstride', target: 'robstride' },
  { label: 'chappe', target: 'chappe' },
  { label: 'systemd', target: 'systemd:' },
] as const;

type LogsArchiveSearchProps = {
  selectedLogId?: string | null;
  onSelectLog?: (entry: LogEntry) => void;
};

function mapProtoLevel(level: string): LogLevel {
  switch (level.toLowerCase()) {
    case 'trace':
    case 'debug':
      return 'DEBUG';
    case 'warn':
      return 'WARN';
    case 'error':
      return 'ERROR';
    case 'fatal':
      return 'FATAL';
    default:
      return 'INFO';
  }
}

function dtoToEntry(row: Awaited<ReturnType<typeof fetchStructuredLogs>>['entries'][number]): LogEntry {
  return {
    id: `search-${row.id}`,
    timestamp: row.timestamp_ms,
    level: mapProtoLevel(row.level),
    source: row.target,
    message: row.message,
    fieldsJson: row.fields_json || undefined,
    sessionId: row.session_id || undefined,
    storeId: row.id,
  };
}

export function LogsArchiveSearch({
  selectedLogId = null,
  onSelectLog,
}: LogsArchiveSearchProps) {
  const [query, setQuery] = useState('');
  const [level, setLevel] = useState('');
  const [target, setTarget] = useState('');
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const runSearch = useCallback(async () => {
    setLoading(true);
    setError(null);
    const result = await fetchStructuredLogs({
      q: query.trim() || undefined,
      level: level || undefined,
      limit: 200,
    });
    const filtered = result.entries.filter((row) => {
      if (!target) {
        return true;
      }
      if (target.endsWith(':')) {
        return row.target.startsWith(target);
      }
      return row.target.includes(target);
    });
    setEntries(filtered.map(dtoToEntry));
    setTotal(result.total);
    setLoading(false);
    if (result.entries.length === 0 && !query.trim()) {
      setError('No structured logs in SQLite yet.');
    }
  }, [query, level, target]);

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-end gap-2">
        <div className="min-w-[200px] flex-1">
          <label className="mb-1 block text-xs text-muted-foreground">FTS query</label>
          <Input
            variant="glass"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="joint, error, operator…"
            className="h-8 font-mono text-xs"
          />
        </div>
        <div>
          <label className="mb-1 block text-xs text-muted-foreground">Level</label>
          <Tabs value={level || 'all'} onValueChange={(v) => setLevel(v === 'all' ? '' : v)}>
            <TabsList variant={logsTabsVariant} className="h-8">
              <TabsTrigger variant={logsTabsVariant} value="all" className="px-2 text-xs">
                all
              </TabsTrigger>
              <TabsTrigger variant={logsTabsVariant} value="info" className="px-2 text-xs">
                info
              </TabsTrigger>
              <TabsTrigger variant={logsTabsVariant} value="warn" className="px-2 text-xs">
                warn
              </TabsTrigger>
              <TabsTrigger variant={logsTabsVariant} value="error" className="px-2 text-xs">
                error
              </TabsTrigger>
              <TabsTrigger variant={logsTabsVariant} value="debug" className="px-2 text-xs">
                debug
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </div>
        <Button type="button" size="sm" onClick={() => void runSearch()} disabled={loading}>
          {loading ? 'Searching…' : 'Search'}
        </Button>
      </div>
      <div className="flex flex-wrap gap-1">
        {SOURCE_PRESETS.map((preset) => (
          <Button
            key={preset.label}
            type="button"
            size="sm"
            variant={target === preset.target ? 'default' : 'outline'}
            className="h-7 px-2 text-xs"
            onClick={() => setTarget(preset.target)}
          >
            {preset.label}
          </Button>
        ))}
      </div>
      {error ? <p className="text-sm text-muted-foreground">{error}</p> : null}
      <p className="text-xs text-muted-foreground">
        {entries.length} shown · {total} total matches (FTS over message, target, fields)
      </p>
      <div className="relative min-h-0 flex-1 overflow-auto rounded-lg border bg-card/95">
        {entries.length === 0 ? (
          <p className="p-4 text-sm text-muted-foreground">
            Search SQLite structured logs across sessions.
          </p>
        ) : (
          <div>
            {entries.map((entry) => (
              <LogRow
                key={entry.id}
                entry={entry}
                positioned={false}
                selected={entry.id === selectedLogId}
                onSelect={onSelectLog}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
