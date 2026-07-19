import {
  logErrorMessage,
  shouldShowLogErrorBanner,
  type LogApiError,
  type LogSessionDto,
} from '@/lib/log-api';
import { logsSessionListShellClassName } from '@/components/dashboard/logs/constants';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

type Props = {
  sessions: LogSessionDto[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  error?: LogApiError | null;
  loading?: boolean;
};

const HOT_WINDOW_MS = 24 * 60 * 60 * 1000;

function sessionLifecycle(session: LogSessionDto, index: number): 'hot' | 'archived' {
  const age = Date.now() - Number(session.started_ms);
  if (index === 0 && age < HOT_WINDOW_MS) {
    return 'hot';
  }
  return 'archived';
}

export function LogsSessionList({ sessions, selectedId, onSelect, error = null, loading = false }: Props) {
  if (shouldShowLogErrorBanner(error)) {
    return (
      <div className={cn(logsSessionListShellClassName, 'p-4 text-sm text-destructive')}>
        {logErrorMessage(error!)}
      </div>
    );
  }

  if (loading) {
    return (
      <div className={cn(logsSessionListShellClassName, 'p-4 text-sm text-muted-foreground')}>
        Loading sessions…
      </div>
    );
  }

  if (sessions.length === 0) {
    return (
      <div className={cn(logsSessionListShellClassName, 'p-4 text-sm text-muted-foreground')}>
        No archived sessions.
      </div>
    );
  }

  return (
    <div className={logsSessionListShellClassName} data-testid="logs-session-list-shell">
      {sessions.map((session, index) => {
        const lifecycle = sessionLifecycle(session, index);
        return (
          <button
            key={session.id}
            type="button"
            onClick={() => onSelect(session.id)}
            className={cn(
              'rounded-md bg-card/95 px-2 py-2 text-left text-sm hover:bg-muted',
              selectedId === session.id && 'bg-muted font-medium',
            )}
          >
            <div className="flex items-center gap-2">
              <span className="truncate">{session.label || session.id}</span>
              <Badge
                variant={lifecycle === 'hot' ? 'secondary' : 'outline'}
                className="px-1 py-0 text-[10px] uppercase"
              >
                {lifecycle}
              </Badge>
            </div>
            <div className="text-xs text-muted-foreground">
              {new Date(Number(session.started_ms)).toLocaleString()}
              {session.has_candump ? ' · CAN' : ''}
              {session.has_trace ? ' · trace' : ''}
            </div>
          </button>
        );
      })}
    </div>
  );
}
