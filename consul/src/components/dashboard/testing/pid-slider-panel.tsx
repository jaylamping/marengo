import * as React from 'react';
import { useTestingStore } from '@/state/testingStore';
import { Slider } from '@/components/ui/slider';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import { useConfigSnapshot } from '@/hooks/use-config-snapshot';

export function PidSliderPanel() {
  const { selectedJointNames, gains, setGain, dispatchGainUpdate } = useTestingStore();
  const { data: config = null } = useConfigSnapshot();

  // Practical tuning limits per Robstride motor type.
  const GAIN_LIMITS: Record<string, { kp_max: number; kd_max: number; ki_max: number; fc_max: number }> = {
    rs00: { kp_max: 200,  kd_max: 5,   ki_max: 20,  fc_max: 10 },
    rs02: { kp_max: 200,  kd_max: 5,   ki_max: 20,  fc_max: 10 },
    rs03: { kp_max: 500,  kd_max: 20,  ki_max: 50,  fc_max: 30 },
    rs04: { kp_max: 500,  kd_max: 20,  ki_max: 50,  fc_max: 60 },
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
      {selectedJointNames.map((jointName) => {
        const gain = gains[jointName] || { kp: 0, kd: 0, ki: 0, fc: 0 };
        const motorConfig = config?.motors.find(m => m.joint === jointName);
        const limits = GAIN_LIMITS[motorConfig?.motor_type ?? 'rs03'] ?? GAIN_LIMITS.rs03;

        const sliders = [
          { key: 'kp' as const, label: 'Kp', max: limits.kp_max },
          { key: 'kd' as const, label: 'Kd', max: limits.kd_max },
          { key: 'ki' as const, label: 'Ki', max: limits.ki_max },
          { key: 'fc' as const, label: 'Fc', max: limits.fc_max },
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
                      value={[gain[key]]}
                      min={0}
                      max={max}
                      step={max / 200}
                      onValueChange={(v) => {
                        setGain(jointName, { ...gain, [key]: v[0] });
                        dispatchGainUpdate(jointName);
                      }}
                      className="flex-1"
                    />
                    <input
                      type="number"
                      min={0}
                      max={max}
                      step={max / 200}
                      value={gain[key]}
                      onChange={(e) => {
                        const v = Math.max(0, Math.min(max, Number(e.target.value) || 0));
                        setGain(jointName, { ...gain, [key]: v });
                        dispatchGainUpdate(jointName);
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
