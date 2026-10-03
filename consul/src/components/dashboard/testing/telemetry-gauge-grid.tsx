import * as React from 'react';
import { useRobotStore } from '@/state/robotStore';
import { useTestingStore } from '@/state/testingStore';
import { useCompoundStore } from '@/state/compoundStore';
import { useActuatorZeroStore } from '@/state/actuatorZeroStore';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { cn } from '@/lib/utils';
import { formatSigFig } from '@/lib/format';
import {
  badgeToneClass,
  resolveActuatorCardBadges,
} from '@/lib/actuator-card-badges';
import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import { compoundPresetById } from '@/data/compound-tests';
import { useConfigSnapshot } from '@/hooks/use-config-snapshot';
import { liveJointEnvelope, useActuatorStore } from '@/state/actuatorStore';

type GaugeLimits = {
  /** Absolute torque cap (Nm), or null when neither Davout nor config publishes one. */
  torqueLimitNm: number | null;
  /** Absolute velocity cap (rad/s), or null when unknown. */
  velocityMaxRadS: number | null;
  /** Hard position range (rad), or null when unknown. */
  position: { lower: number; upper: number } | null;
};

/**
 * Gauge limits come from the live Davout snapshot first, then the master
 * config snapshot. There is no per-motor-type fallback table: an unknown cap
 * renders as unknown, never as a plausible-looking wrong number.
 */
export function resolveGaugeLimits(input: {
  live: { tauFfMaxNm: number; velocityMaxRadS: number } | null;
  envelope: { hardLowerRad: number; hardUpperRad: number } | null;
  benchTorqueNm: number | undefined;
  benchPosition: { lower: number; upper: number } | undefined;
  configVelocityMaxRadS: number | undefined;
}): GaugeLimits {
  const finite = (v: number | undefined): number | null =>
    v !== undefined && Number.isFinite(v) && v > 0 ? v : null;
  const position = input.envelope
    ? { lower: input.envelope.hardLowerRad, upper: input.envelope.hardUpperRad }
    : (input.benchPosition ?? null);
  return {
    torqueLimitNm: finite(input.live?.tauFfMaxNm) ?? finite(input.benchTorqueNm),
    velocityMaxRadS:
      finite(input.live?.velocityMaxRadS) ?? finite(input.configVelocityMaxRadS),
    position:
      position && Number.isFinite(position.lower) && Number.isFinite(position.upper)
        && position.upper > position.lower
        ? position
        : null,
  };
}

export function TelemetryGaugeGrid() {
  const robotState = useRobotStore((s) => s.robotState);
  const operationalMode = useRobotStore((s) => s.operationalMode);
  const zeroed = useActuatorZeroStore((s) => s.zeroed);
  const { selectedJointNames } = useTestingStore();
  const { selectedPresetId } = useCompoundStore();
  const { data: config = null } = useConfigSnapshot();
  const limitSnapshot = useActuatorStore((s) => s.limitSnapshot);

  if (!robotState) return null;

  let jointsToDisplay = selectedJointNames;
  if (selectedPresetId) {
    const preset = compoundPresetById(selectedPresetId);
    if (preset) {
      jointsToDisplay = preset.joints;
    }
  }

  if (jointsToDisplay.length === 0) {
    return (
      <Card variant="panel" className={dashboardPanelCardClassName}>
        <CardContent className="py-8 text-center text-muted-foreground text-sm">
          Select actuators to view telemetry.
        </CardContent>
      </Card>
    );
  }

  const jointsToShow = robotState.joints.filter((j) =>
    jointsToDisplay.includes(j.name),
  );

  return (
    <div className="grid gap-4">
      {jointsToShow.map((joint) => {
        const motorConfig = config?.motors.find((m) => m.joint === joint.name);
        const controlLimit = config?.control_limits.find(
          (c) => c.joint === joint.name,
        );
        const liveCaps = limitSnapshot?.joints.find((j) => j.joint === joint.name);
        const limits = resolveGaugeLimits({
          live: liveCaps ?? null,
          envelope: liveJointEnvelope(joint.name, limitSnapshot),
          benchTorqueNm: motorConfig?.bench.torque_limit_nm,
          benchPosition: motorConfig
            ? {
                lower: motorConfig.bench.position_lower_rad,
                upper: motorConfig.bench.position_upper_rad,
              }
            : undefined,
          configVelocityMaxRadS: controlLimit?.velocity_max_rad_s,
        });
        const { torqueLimitNm, velocityMaxRadS, position } = limits;
        const posPercent = position
          ? Math.abs((joint.position - position.lower) / (position.upper - position.lower)) * 100
          : null;
        const torquePercent =
          torqueLimitNm === null ? null : Math.abs(joint.effort / torqueLimitNm) * 100;
        const velPercent =
          velocityMaxRadS === null ? null : Math.abs(joint.velocity / velocityMaxRadS) * 100;

        const badges = resolveActuatorCardBadges({
          operationalMode,
          zeroed: Boolean(zeroed[joint.name]),
          fault: joint.fault,
        });

        return (
          <Card
            key={joint.name}
            variant="panel"
            className={dashboardPanelCardClassName}
          >
            <CardHeader className="flex flex-row items-start justify-between gap-2 space-y-0 pb-2">
              <CardTitle className="text-sm uppercase tracking-wide">
                {joint.name}
              </CardTitle>
              <div className="flex flex-wrap items-center justify-end gap-1">
                {badges.map((badge) => (
                  <Badge
                    key={badge.id}
                    variant="outline"
                    className={cn(
                      'h-5 px-1.5 uppercase tracking-[0.12em]',
                      badgeToneClass(badge.tone),
                    )}
                  >
                    {badge.label}
                  </Badge>
                ))}
              </div>
            </CardHeader>
            <CardContent className="space-y-2">
              <Gauge
                label="Position"
                value={joint.position}
                percent={posPercent}
                unit="rad"
                limit={
                  position
                    ? `${position.lower.toFixed(2)} → ${position.upper.toFixed(2)}`
                    : 'limit unknown'
                }
              />
              <Gauge
                label="Velocity"
                value={joint.velocity}
                percent={velPercent}
                unit="rad/s"
                limit={velocityMaxRadS === null ? 'limit unknown' : `±${velocityMaxRadS.toFixed(2)}`}
              />
              <Gauge
                label="Torque"
                value={joint.effort}
                percent={torquePercent}
                unit="Nm"
                limit={torqueLimitNm === null ? 'limit unknown' : `±${torqueLimitNm.toFixed(2)}`}
              />
              <div className="data-value text-xs text-muted-foreground">
                TEMP{' '}
                <span className="text-foreground">
                  {joint.temperatureC?.toFixed(1) ?? '—'}°C
                </span>
              </div>
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}

function Gauge({
  label,
  value,
  percent,
  unit,
  limit,
}: {
  label: string;
  value: number;
  /** Null when the limit is unknown: the bar is hidden, never drawn at 0 %. */
  percent: number | null;
  unit: string;
  limit: string;
}) {
  return (
    <div className="space-y-1">
      <div className="flex justify-between text-xs">
        <span className="micro-label">{label}</span>
        <span className="data-value">
          {formatSigFig(value)} {unit}{' '}
          <span className="text-muted-foreground">/ {limit}</span>
        </span>
      </div>
      {percent === null ? (
        <div className="h-1.5 rounded-sm bg-surface-3" data-testid="gauge-unknown" />
      ) : (
        <div className="h-1.5 rounded-sm bg-surface-3 overflow-hidden">
          <div
            className={cn(
              'h-full transition-all',
              percent > 90 ? 'bg-fault' : percent > 70 ? 'bg-warning' : 'bg-ok',
            )}
            style={{ width: `${Math.min(percent, 100)}%` }}
          />
        </div>
      )}
    </div>
  );
}
