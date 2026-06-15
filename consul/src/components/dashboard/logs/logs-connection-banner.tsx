import { useHostMetricsStore } from '@/state/hostMetricsStore';
import { useRobotStore } from '@/state/robotStore';
import { isChappeLive } from '@/lib/chappe-config';
import { cn } from '@/lib/utils';

export function LogsConnectionBanner() {
  const transportMode = useHostMetricsStore((s) => s.transportMode);
  const connected = useRobotStore((s) => s.connected);
  const gatewayError = useRobotStore((s) => s.gatewayError);
  const piMetrics = useHostMetricsStore((s) => s.piMetrics);

  const mode = !isChappeLive()
    ? 'offline'
    : connected
      ? transportMode
      : 'reconnecting';

  const logDisk = piMetrics?.logDiskBytes;
  const logBudget = piMetrics?.logDiskBudgetBytes;

  return (
    <div className="flex flex-wrap items-center gap-3 rounded-lg border bg-muted/30 px-3 py-2 text-sm">
      <span
        className={cn(
          'inline-flex items-center gap-1.5 font-medium',
          mode === 'webtransport' && 'text-emerald-600',
          mode === 'http-stream' && 'text-amber-600',
          (mode === 'offline' || mode === 'reconnecting') && 'text-muted-foreground',
        )}
      >
        <span
          className={cn(
            'size-2 rounded-full',
            mode === 'webtransport' && 'bg-emerald-500',
            mode === 'http-stream' && 'bg-amber-500',
            mode === 'reconnecting' && 'bg-amber-400 animate-pulse',
            mode === 'offline' && 'bg-muted-foreground',
          )}
        />
        {mode}
      </span>
      {gatewayError ? (
        <span className="text-destructive">{gatewayError}</span>
      ) : null}
      {logDisk !== undefined && logBudget !== undefined && logBudget > 0n ? (
        <span className="text-muted-foreground">
          log disk {Math.round(Number((logDisk * 100n) / logBudget))}%
        </span>
      ) : null}
    </div>
  );
}
