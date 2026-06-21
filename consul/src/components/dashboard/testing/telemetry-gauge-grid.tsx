import * as React from 'react';
import { useRobotStore } from '@/state/robotStore';
import { useTestingStore } from '@/state/testingStore';
import { fetchConfigSnapshot, ConfigSnapshotDto } from '@/lib/config-api';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { cn } from '@/lib/utils';
import { formatSigFig } from '@/lib/format';

const GAIN_LIMITS: Record<string, { kp_max: number; kd_max: number; tau_ff_max_nm: number; velocity_max_rad_s: number }> = {
  rs00: { kp_max: 500, kd_max: 5, tau_ff_max_nm: 17, velocity_max_rad_s: 50 },
  rs02: { kp_max: 500, kd_max: 5, tau_ff_max_nm: 17, velocity_max_rad_s: 44 },
  rs03: { kp_max: 5000, kd_max: 100, tau_ff_max_nm: 60, velocity_max_rad_s: 50 },
  rs04: { kp_max: 5000, kd_max: 100, tau_ff_max_nm: 120, velocity_max_rad_s: 15 },
};

export function TelemetryGaugeGrid() {
  const { robotState } = useRobotStore();
  const { selectedJointNames } = useTestingStore();
  const [config, setConfig] = React.useState<ConfigSnapshotDto | null>(null);

  React.useEffect(() => {
    fetchConfigSnapshot().then(setConfig);
  }, []);

  if (!robotState || selectedJointNames.length === 0) return null;

  const jointsToShow = robotState.joints.filter(j => selectedJointNames.includes(j.name));

  return (
    <div className="grid gap-4">
      {jointsToShow.map((joint) => {
        const motorConfig = config?.motors.find(m => m.joint === joint.name);
        const controlLimit = config?.control_limits.find(c => c.joint === joint.name);
        const limits = GAIN_LIMITS[motorConfig?.motor_type ?? 'rs03'] ?? GAIN_LIMITS.rs03;
        const torqueLimit = motorConfig?.bench.torque_limit_nm ?? limits.tau_ff_max_nm;
        const posUpper = motorConfig?.bench.position_upper_rad ?? Math.PI;
        const posLower = motorConfig?.bench.position_lower_rad ?? -Math.PI;
        const velMax = controlLimit?.velocity_max_rad_s ?? limits.velocity_max_rad_s;

        const posRange = posUpper - posLower;
        const posPercent = posRange > 0 ? Math.abs((joint.position - posLower) / posRange) * 100 : 0;
        const torquePercent = torqueLimit > 0 ? Math.abs(joint.effort / torqueLimit) * 100 : 0;
        const velPercent = velMax > 0 ? Math.abs(joint.velocity / velMax) * 100 : 0;

        return (
          <Card key={joint.name}>
            <CardHeader><CardTitle className="text-sm">{joint.name}</CardTitle></CardHeader>
            <CardContent className="space-y-2">
              <Gauge label="Position" value={joint.position} percent={posPercent} unit="rad" limit={`${posLower.toFixed(2)} → ${posUpper.toFixed(2)}`} />
              <Gauge label="Velocity" value={joint.velocity} percent={velPercent} unit="rad/s" limit={`±${velMax.toFixed(2)}`} />
              <Gauge label="Torque" value={joint.effort} percent={torquePercent} unit="Nm" limit={`±${torqueLimit.toFixed(2)}`} />
              <div className="text-xs">Temp: {joint.temperatureC?.toFixed(1) ?? '—'}°C</div>
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}

function Gauge({ label, value, percent, unit, limit }: { label: string, value: number, percent: number, unit: string, limit: string }) {
  return (
    <div className="space-y-1">
      <div className="flex justify-between text-xs">
        <span>{label}</span>
        <span>{formatSigFig(value)} {unit} <span className="text-muted-foreground">/ {limit}</span></span>
      </div>
      <div className="h-2 bg-muted rounded-full overflow-hidden">
        <div
          className={cn("h-full transition-all", percent > 90 ? "bg-red-500" : percent > 70 ? "bg-yellow-500" : "bg-green-500")}
          style={{ width: `${Math.min(percent, 100)}%` }}
        />
      </div>
    </div>
  );
}
