import { siteHeaderConfig } from '@/data/site-header';
import { isChappeLive } from '@/lib/chappe-config';
import { cn } from '@/lib/utils';
import { useHostMetricsStore } from '@/state/hostMetricsStore';
import { useRobotStore } from '@/state/robotStore';

type MachineState = {
  label: string;
  ledClassName: string;
  textClassName: string;
};

function resolveMachineState(
  live: boolean,
  connected: boolean,
  operationalMode: string | null,
  gatewayError: string | null,
): MachineState {
  if (!live) {
    return {
      label: 'WIREFRAME',
      ledClassName: 'led',
      textClassName: 'text-muted-foreground',
    };
  }
  if (!connected) {
    return gatewayError
      ? { label: 'CHAPPE ERR', ledClassName: 'led led-fault', textClassName: 'text-fault' }
      : { label: 'CONNECTING', ledClassName: 'led led-accent', textClassName: 'text-muted-foreground' };
  }
  const mode = operationalMode ?? 'LIVE';
  if (mode === 'ACTIVE') {
    return { label: mode, ledClassName: 'led led-accent led-live', textClassName: 'text-accent' };
  }
  if (mode === 'READY') {
    return { label: mode, ledClassName: 'led led-ok', textClassName: 'text-foreground' };
  }
  return { label: mode, ledClassName: 'led', textClassName: 'text-muted-foreground' };
}

export function SiteHeaderStatusBadges() {
  const connected = useRobotStore((s) => s.connected);
  const operationalMode = useRobotStore((s) => s.operationalMode);
  const gatewayError = useRobotStore((s) => s.gatewayError);
  const transportMode = useHostMetricsStore((s) => s.transportMode);
  const live = isChappeLive();

  const machine = resolveMachineState(live, connected, operationalMode, gatewayError);

  return (
    <div className="flex items-center gap-2">
      <span className="micro-label hidden border border-line bg-surface-0 px-2 py-1 leading-none sm:inline-flex">
        {siteHeaderConfig.bus}
      </span>
      {live && transportMode !== 'offline' ? (
        <span className="micro-label inline-flex items-center gap-1.5 border border-line bg-surface-0 px-2 py-1 leading-none">
          <span className={connected ? 'led led-ok' : 'led'} aria-hidden />
          {transportMode === 'webtransport' ? 'WT' : 'HTTP'}
        </span>
      ) : null}
      {/* Machine state — the one element in the header that must read at a glance. */}
      <span
        data-testid="machine-state"
        className={cn(
          'inline-flex items-center gap-2 border px-2.5 py-1',
          machine.textClassName === 'text-accent'
            ? 'border-accent/40 bg-surface-0'
            : 'border-line bg-surface-0',
        )}
      >
        <span className={machine.ledClassName} aria-hidden />
        <span
          className={cn(
            'font-mono text-xs font-medium uppercase tracking-[0.14em] leading-none',
            machine.textClassName,
          )}
        >
          {machine.label}
        </span>
      </span>
    </div>
  );
}
