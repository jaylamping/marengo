import { useCallback, useState } from 'react';

import { LogRow } from '@/components/dashboard/logs/log-row';
import type { LogEntry, LogLevel } from '@/data/logs';
import { fetchStructuredLogs } from '@/lib/log-api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

const SOURCE_PRESETS = [
  { label: 'All', target: '' },
  { label: 'davout', target: 'davout' },
  { label: 'berthier', target: 'berthier' },
  { label: 'robstride', target: 'robstride' },
  { label: 'chappe', target: 'chappe' },
  { label: 'systemd', target: 'systemd:' },
] as const;

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
  };
}

export function LogsArchiveSearch() {
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
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="joint, error, operator…"
            className="h-8 font-mono text-xs"
          />
        </div>
        <div>
          <label className="mb-1 block text-xs text-muted-foreground">Level</label>
          <select
            value={level}
            onChange={(e) => setLevel(e.target.value)}
            className="h-8 rounded-md border bg-background px-2 text-xs"
          >
            <option value="">all</option>
            <option value="info">info</option>
            <option value="warn">warn</option>
            <option value="error">error</option>
            <option value="debug">debug</option>
          </select>
        </div>
        <Button type="button" size="sm" onClick={() => void runSearch()} disabled={loading}>
          {loading ? 'Searching…' : 'Search'}
        </Button>
      </div>
      <div className="flex flex-wrap gap-1">
        {SOURCE_PRESETS.map((preset) => (
          <button
            key={preset.label}
            type="button"
            onClick={() => setTarget(preset.target)}
            className={`rounded px-2 py-0.5 text-xs ${
              target === preset.target ? 'bg-primary text-primary-foreground' : 'bg-muted'
            }`}
          >
            {preset.label}
          </button>
        ))}
      </div>
      {error ? <p className="text-sm text-muted-foreground">{error}</p> : null}
      <p className="text-xs text-muted-foreground">
        {entries.length} shown · {total} total matches (FTS over message, target, fields)
      </p>
      <div className="relative min-h-0 flex-1 overflow-auto rounded-lg border bg-card">
        {entries.length === 0 ? (
          <p className="p-4 text-sm text-muted-foreground">
            Search SQLite structured logs across sessions.
          </p>
        ) : (
          <div>
            {entries.map((entry) => (
              <LogRow key={entry.id} entry={entry} positioned={false} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
