import type { LogSessionDto } from '@/lib/log-api';
import { cn } from '@/lib/utils';

type Props = {
  sessions: LogSessionDto[];
  selectedId: string | null;
  onSelect: (id: string) => void;
};

export function LogsSessionList({ sessions, selectedId, onSelect }: Props) {
  if (sessions.length === 0) {
    return (
      <div className="rounded-lg border p-4 text-sm text-muted-foreground">
        No archived sessions.
      </div>
    );
  }

  return (
    <div className="flex max-h-[480px] flex-col gap-1 overflow-auto rounded-lg border p-2">
      {sessions.map((session) => (
        <button
          key={session.id}
          type="button"
          onClick={() => onSelect(session.id)}
          className={cn(
            'rounded-md px-2 py-2 text-left text-sm hover:bg-muted',
            selectedId === session.id && 'bg-muted font-medium',
          )}
        >
          <div className="truncate">{session.label || session.id}</div>
          <div className="text-xs text-muted-foreground">
            {new Date(Number(session.started_ms)).toLocaleString()}
            {session.has_candump ? ' · CAN' : ''}
            {session.has_trace ? ' · trace' : ''}
          </div>
        </button>
      ))}
    </div>
  );
}
