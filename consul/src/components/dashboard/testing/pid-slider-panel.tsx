import * as React from 'react';
import { useTestingStore } from '@/state/testingStore';
import { fetchConfigSnapshot, ConfigSnapshotDto } from '@/lib/config-api';
import { Slider } from '@/components/ui/slider';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';

export function PidSliderPanel() {
  const { selectedJointNames, gains, setGain, dispatchGainUpdate } = useTestingStore();
  const [config, setConfig] = React.useState<ConfigSnapshotDto | null>(null);

  React.useEffect(() => {
    fetchConfigSnapshot().then(setConfig);
  }, []);

  // Hardware-scale gain limits per Robstride motor type (from robstride/motor_type.rs MitRanges).
  const GAIN_LIMITS: Record<string, { kp_max: number; kd_max: number; tau_ff_max_nm: number }> = {
    rs00: { kp_max: 500,   kd_max: 5,   tau_ff_max_nm: 17 },
    rs02: { kp_max: 500,   kd_max: 5,   tau_ff_max_nm: 17 },
    rs03: { kp_max: 5000,  kd_max: 100, tau_ff_max_nm: 60 },
    rs04: { kp_max: 5000,  kd_max: 100, tau_ff_max_nm: 120 },
  };

  return (
    <div className="flex flex-col gap-4">
      {selectedJointNames.map((jointName) => {
        const gain = gains[jointName] || { kp: 0, kd: 0, ki: 0, fc: 0 };
        const motorConfig = config?.motors.find(m => m.joint === jointName);
        const limits = GAIN_LIMITS[motorConfig?.motor_type ?? 'rs03'] ?? GAIN_LIMITS.rs03;

        const sliders = [
          { key: 'kp', label: 'Kp', max: limits.kp_max },
          { key: 'kd', label: 'Kd', max: limits.kd_max },
          { key: 'ki', label: 'Ki', max: limits.kp_max },
          { key: 'fc', label: 'Fc', max: limits.tau_ff_max_nm },
        ] as const;

        return (
          <Card key={jointName}>
            <CardHeader><CardTitle>{jointName}</CardTitle></CardHeader>
            <CardContent className="grid grid-cols-2 gap-4">
              {sliders.map(({ key, label, max }) => (
                <div key={key} className="space-y-1">
                  <div className="flex justify-between text-xs">
                    <span>{label}</span>
                    <span>{gain[key].toFixed(2)} / {max.toFixed(2)}</span>
                  </div>
                  <Slider
                    value={[gain[key]]}
                    min={0}
                    max={max}
                    step={max / 100}
                    onValueChange={(v) => {
                      setGain(jointName, { ...gain, [key]: v[0] });
                      dispatchGainUpdate(jointName);
                    }}
                  />
                </div>
              ))}
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}
