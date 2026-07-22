import type { ReactNode } from 'react';

import { NeedsRestartBadge } from '@/components/dashboard/needs-restart/needs-restart-badge';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { siteHeaderConfig } from '@/data/site-header';
import {
  chappeConnectionErrDetail,
  chappeMisconfigHint,
  resolveChappeEndpoints,
} from '@/lib/chappe-config';
import { cn } from '@/lib/utils';
import { useHostMetricsStore } from '@/state/hostMetricsStore';
import type { ChappeTransportMode } from '@/state/hostMetricsStore';
import { useRobotStore } from '@/state/robotStore';

type MachineState = {
  label: string;
  ledClassName: string;
  textClassName: string;
  /** One-line meaning of the current label. */
  summary: string;
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
      summary: 'No Chappe endpoints configured — UI is offline / local only.',
    };
  }
  if (!connected) {
    return gatewayError
      ? {
          label: 'CHAPPE ERR',
          ledClassName: 'led led-fault',
          textClassName: 'text-fault',
          summary: 'Gateway / Chappe connection failed.',
        }
      : {
          label: 'CONNECTING',
          ledClassName: 'led led-accent',
          textClassName: 'text-muted-foreground',
          summary: 'Endpoints configured; waiting for gateway telemetry.',
        };
  }
  const mode = operationalMode ?? 'LIVE';
  if (mode === 'ACTIVE') {
    return {
      label: mode,
      ledClassName: 'led led-accent led-live',
      textClassName: 'text-accent',
      summary: 'Motors enabled — Davout Active (torque / tracking allowed).',
    };
  }
  if (mode === 'READY') {
    return {
      label: mode,
      ledClassName: 'led led-ok',
      textClassName: 'text-foreground',
      summary: 'Homing verified — ready to enable; motors still off.',
    };
  }
  if (mode === 'DISABLED') {
    return {
      label: mode,
      ledClassName: 'led',
      textClassName: 'text-muted-foreground',
      summary: 'Motors disabled — no torque from software enable.',
    };
  }
  return {
    label: mode,
    ledClassName: 'led',
    textClassName: 'text-muted-foreground',
    summary: 'Connected; operational mode unknown or transitional.',
  };
}

function transportTooltipBody(mode: ChappeTransportMode, connected: boolean) {
  const current =
    mode === 'webtransport'
      ? connected
        ? 'Now: WT — WebTransport linked (preferred).'
        : 'Now: WT — WebTransport selected; link not up yet.'
      : mode === 'http-stream'
        ? connected
          ? 'Now: HTTP — streaming over HTTP (fallback).'
          : 'Now: HTTP — stream selected; link not up yet.'
        : 'Now: offline — no live transport.';

  return (
    <div className="flex flex-col gap-1.5 text-left">
      <p className="font-medium text-foreground">Consul ↔ gateway transport</p>
      <p className="text-foreground/90">{current}</p>
      <p className="text-muted-foreground">
        Green LED = Chappe telemetry connected; dim = endpoints up but not
        linked yet.
      </p>
      <ul className="list-disc space-y-0.5 pl-3.5 text-muted-foreground">
        <li>
          <span className="font-mono text-foreground/90">WT</span> — WebTransport
          (QUIC); primary live path
        </li>
        <li>
          <span className="font-mono text-foreground/90">HTTP</span> — HTTP stream
          fallback when WT is unavailable
        </li>
        <li>Hidden when endpoints are unset or transport is offline</li>
      </ul>
    </div>
  );
}

function machineTooltipBody(machine: MachineState, errDetail: string | null) {
  return (
    <div className="flex flex-col gap-1.5 text-left">
      <p className="font-medium text-foreground">Machine state (Davout)</p>
      <p className="text-foreground/90">
        Now: <span className="font-mono">{machine.label}</span> — {machine.summary}
      </p>
      {errDetail ? (
        <p className="break-all text-muted-foreground">{errDetail}</p>
      ) : null}
      <ul className="list-disc space-y-0.5 pl-3.5 text-muted-foreground">
        <li>
          <span className="font-mono text-foreground/90">WIREFRAME</span> — no
          Chappe endpoints
        </li>
        <li>
          <span className="font-mono text-foreground/90">CONNECTING</span> — waiting
          for gateway
        </li>
        <li>
          <span className="font-mono text-foreground/90">CHAPPE ERR</span> —
          connection failed
        </li>
        <li>
          <span className="font-mono text-foreground/90">DISABLED</span> — motors
          off
        </li>
        <li>
          <span className="font-mono text-foreground/90">READY</span> — verified,
          safe to enable
        </li>
        <li>
          <span className="font-mono text-foreground/90">ACTIVE</span> — motors
          enabled
        </li>
      </ul>
    </div>
  );
}

function HeaderBadgeTooltip({
  ariaLabel,
  content,
  children,
}: {
  ariaLabel: string;
  content: ReactNode;
  children: ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          tabIndex={0}
          className="cursor-help outline-none focus-visible:ring-2 focus-visible:ring-ring"
          aria-label={ariaLabel}
        >
          {children}
        </span>
      </TooltipTrigger>
      <TooltipContent
        side="bottom"
        align="end"
        className="max-w-xs py-2 leading-relaxed"
      >
        {content}
      </TooltipContent>
    </Tooltip>
  );
}

export function SiteHeaderStatusBadges() {
  const connected = useRobotStore((s) => s.connected);
  const operationalMode = useRobotStore((s) => s.operationalMode);
  const gatewayError = useRobotStore((s) => s.gatewayError);
  const transportMode = useHostMetricsStore((s) => s.transportMode);
  const resolution = resolveChappeEndpoints();
  const live = resolution.endpoints !== null;
  const chappeErrDetail =
    live && !connected && gatewayError
      ? chappeConnectionErrDetail(resolution, chappeMisconfigHint())
      : null;

  const machine = resolveMachineState(live, connected, operationalMode, gatewayError);

  return (
    <div className="flex items-center gap-2">
      <NeedsRestartBadge variant="needs" />
      <HeaderBadgeTooltip
        ariaLabel={`CAN bus ${siteHeaderConfig.bus}`}
        content={
          <div className="flex flex-col gap-1.5 text-left">
            <p className="font-medium text-foreground">Bench CAN interface</p>
            <p className="text-foreground/90">
              Configured SocketCAN name for this profile (
              <span className="font-mono">{siteHeaderConfig.bus}</span>).
            </p>
            <p className="text-muted-foreground">
              Static label from UI config — not live link state. Live CAN health
              is under host metrics (ERROR-ACTIVE is healthy; bus-off / error
              passive warn).
            </p>
          </div>
        }
      >
        <span className="micro-label hidden border border-line bg-surface-0 px-2 py-1 leading-none sm:inline-flex">
          {siteHeaderConfig.bus}
        </span>
      </HeaderBadgeTooltip>
      {live && transportMode !== 'offline' ? (
        <HeaderBadgeTooltip
          ariaLabel={
            transportMode === 'webtransport'
              ? 'Transport WebTransport'
              : 'Transport HTTP stream'
          }
          content={transportTooltipBody(transportMode, connected)}
        >
          <span className="micro-label inline-flex items-center gap-1.5 border border-line bg-surface-0 px-2 py-1 leading-none">
            <span className={connected ? 'led led-ok' : 'led'} aria-hidden />
            {transportMode === 'webtransport' ? 'WT' : 'HTTP'}
          </span>
        </HeaderBadgeTooltip>
      ) : null}
      {/* Machine state — the one element in the header that must read at a glance. */}
      <HeaderBadgeTooltip
        ariaLabel={`Machine state ${machine.label}`}
        content={machineTooltipBody(machine, chappeErrDetail)}
      >
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
      </HeaderBadgeTooltip>
    </div>
  );
}
