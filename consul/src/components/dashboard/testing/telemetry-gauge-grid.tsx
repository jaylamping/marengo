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

const GAIN_LIMITS: Record<
  string,
  {
    kp_max: number;
    kd_max: number;
    tau_ff_max_nm: number;
    velocity_max_rad_s: number;
  }
> = {
  rs00: { kp_max: 500, kd_max: 5, tau_ff_max_nm: 17, velocity_max_rad_s: 50 },
  rs02: { kp_max: 500, kd_max: 5, tau_ff_max_nm: 17, velocity_max_rad_s: 44 },
  rs03: {
    kp_max: 5000,
    kd_max: 100,
    tau_ff_max_nm: 60,
    velocity_max_rad_s: 50,
  },
  rs04: {
    kp_max: 5000,
    kd_max: 100,
    tau_ff_max_nm: 120,
    velocity_max_rad_s: 15,
  },
};

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
        const limits =
          GAIN_LIMITS[motorConfig?.motor_type ?? 'rs03'] ?? GAIN_LIMITS.rs03;
        const liveCaps = limitSnapshot?.joints.find((j) => j.joint === joint.name);
        const torqueLimit =
          liveCaps?.tauFfMaxNm ??
          motorConfig?.bench.torque_limit_nm ??
          limits.tau_ff_max_nm;
        const envelope = liveJointEnvelope(joint.name, limitSnapshot);
        const posUpper =
          envelope?.hardUpperRad ??
          motorConfig?.bench.position_upper_rad ??
          Math.PI;
        const posLower =
          envelope?.hardLowerRad ??
          motorConfig?.bench.position_lower_rad ??
          -Math.PI;
        const velMax =
          liveCaps?.velocityMaxRadS ??
          controlLimit?.velocity_max_rad_s ??
          limits.velocity_max_rad_s;

        const posRange = posUpper - posLower;
        const posPercent =
          posRange > 0
            ? Math.abs((joint.position - posLower) / posRange) * 100
            : 0;
        const torquePercent =
          torqueLimit > 0 ? Math.abs(joint.effort / torqueLimit) * 100 : 0;
        const velPercent =
          velMax > 0 ? Math.abs(joint.velocity / velMax) * 100 : 0;

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
                limit={`${posLower.toFixed(2)} → ${posUpper.toFixed(2)}`}
              />
              <Gauge
                label="Velocity"
                value={joint.velocity}
                percent={velPercent}
                unit="rad/s"
                limit={`±${velMax.toFixed(2)}`}
              />
              <Gauge
                label="Torque"
                value={joint.effort}
                percent={torquePercent}
                unit="Nm"
                limit={`±${torqueLimit.toFixed(2)}`}
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
  percent: number;
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
      <div className="h-1.5 rounded-sm bg-surface-3 overflow-hidden">
        <div
          className={cn(
            'h-full transition-all',
            percent > 90 ? 'bg-fault' : percent > 70 ? 'bg-warning' : 'bg-ok',
          )}
          style={{ width: `${Math.min(percent, 100)}%` }}
        />
      </div>
    </div>
  );
}
