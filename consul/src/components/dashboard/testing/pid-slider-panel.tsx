import * as React from 'react';
import { useTestingStore } from '@/state/testingStore';
import { Slider } from '@/components/ui/slider';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import { useConfigSnapshot } from '@/hooks/use-config-snapshot';
import { liveJointLimits, useActuatorStore } from '@/state/actuatorStore';

// Practical tuning limits per Robstride motor type.
const GAIN_LIMITS: Record<string, { kp_max: number; kd_max: number; ki_max: number; fc_max: number }> = {
  rs00: { kp_max: 200,  kd_max: 5,   ki_max: 20,  fc_max: 10 },
  rs02: { kp_max: 200,  kd_max: 5,   ki_max: 20,  fc_max: 10 },
  rs03: { kp_max: 500,  kd_max: 20,  ki_max: 50,  fc_max: 30 },
  rs04: { kp_max: 500,  kd_max: 20,  ki_max: 50,  fc_max: 60 },
};

/** The practical limit, never above the cap the Pi publishes (it clamps there). */
function capped(practical: number, live: number | undefined): number {
  return live !== undefined && Number.isFinite(live) && live > 0
    ? Math.min(practical, live)
    : practical;
}

/**
 * Slider ranges stop at the live ActuatorLimitSnapshot caps, so the panel never
 * shows a gain the Pi clamps away (UI-06). A gain is sent once, when the drag
 * or the typed value is released, not on every tick (UI-06, UI-25).
 */
export function PidSliderPanel() {
  const { selectedJointNames, gains, setGain, dispatchGainUpdate } = useTestingStore();
  const { data: config = null } = useConfigSnapshot();
  const limitSnapshot = useActuatorStore((s) => s.limitSnapshot);
  const [error, setError] = React.useState<string | null>(null);
  // Joints whose gains changed since the last send; release without a change sends nothing.
  const dirty = React.useRef(new Set<string>());

  const edit = (jointName: string, next: { kp: number; kd: number; ki: number; fc: number }) => {
    dirty.current.add(jointName);
    setGain(jointName, next);
  };

  const send = (jointName: string) => {
    if (!dirty.current.delete(jointName)) return;
    setError(null);
    dispatchGainUpdate(jointName).catch((cause: unknown) => {
      setError(cause instanceof Error ? cause.message : String(cause));
    });
  };

  if (selectedJointNames.length === 0) {
    return (
      <Card variant="panel" className={dashboardPanelCardClassName}>
        <CardContent className="py-8 text-center text-muted-foreground text-sm">
          Select actuators to tune PID gains.
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      {error ? (
        <p role="alert" className="text-xs text-destructive">
          {error}
        </p>
      ) : null}
      {selectedJointNames.map((jointName) => {
        const gain = gains[jointName] || { kp: 0, kd: 0, ki: 0, fc: 0 };
        const motorConfig = config?.motors.find(m => m.joint === jointName);
        const limits = GAIN_LIMITS[motorConfig?.motor_type ?? 'rs03'] ?? GAIN_LIMITS.rs03;
        const live = liveJointLimits(jointName, limitSnapshot);

        const sliders = [
          { key: 'kp' as const, label: 'Kp', max: capped(limits.kp_max, live?.kpMax) },
          { key: 'kd' as const, label: 'Kd', max: capped(limits.kd_max, live?.kdMax) },
          { key: 'ki' as const, label: 'Ki', max: limits.ki_max },
          { key: 'fc' as const, label: 'Fc', max: capped(limits.fc_max, live?.tauFfMaxNm) },
        ];

        return (
          <Card key={jointName} variant="panel" className={dashboardPanelCardClassName}>
            <CardHeader className="pb-3"><CardTitle className="text-base">{jointName} Gains</CardTitle></CardHeader>
            <CardContent className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-4">
              {sliders.map(({ key, label, max }) => (
                <div key={key} className="space-y-2">
                  <div className="flex justify-between text-xs font-medium">
                    <span>{label}</span>
                    <span className="text-muted-foreground">{gain[key].toFixed(2)} / {max.toFixed(1)}</span>
                  </div>
                  <div className="flex items-center gap-3">
                    <Slider
                      aria-label={`${jointName} ${label}`}
                      value={[gain[key]]}
                      min={0}
                      max={max}
                      step={max / 200}
                      onValueChange={(v) => {
                        edit(jointName, { ...gain, [key]: Math.min(max, v[0]) });
                      }}
                      onPointerUp={() => send(jointName)}
                      onKeyUp={() => send(jointName)}
                      className="flex-1"
                    />
                    <input
                      type="number"
                      aria-label={`${jointName} ${label} value`}
                      min={0}
                      max={max}
                      step={max / 200}
                      value={gain[key]}
                      onChange={(e) => {
                        const v = Math.max(0, Math.min(max, Number(e.target.value) || 0));
                        edit(jointName, { ...gain, [key]: v });
                      }}
                      onBlur={() => send(jointName)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') send(jointName);
                      }}
                      className="w-16 px-2 py-1 text-xs rounded border border-input bg-background text-right focus:outline-none focus:ring-1 focus:ring-primary"
                    />
                  </div>
                </div>
              ))}
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}
