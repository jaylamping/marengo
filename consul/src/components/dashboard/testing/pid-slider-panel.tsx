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

  // Practical tuning limits per Robstride motor type.
  // Hardware can go higher, but these are the usable ranges for interactive tuning.
  const GAIN_LIMITS: Record<string, { kp_max: number; kd_max: number; ki_max: number; fc_max: number }> = {
    rs00: { kp_max: 200,  kd_max: 5,   ki_max: 20,  fc_max: 10 },
    rs02: { kp_max: 200,  kd_max: 5,   ki_max: 20,  fc_max: 10 },
    rs03: { kp_max: 500,  kd_max: 20,  ki_max: 50,  fc_max: 30 },
    rs04: { kp_max: 500,  kd_max: 20,  ki_max: 50,  fc_max: 60 },
  };

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
          <Card key={jointName}>
            <CardHeader><CardTitle>{jointName}</CardTitle></CardHeader>
            <CardContent className="grid grid-cols-2 gap-4">
              {sliders.map(({ key, label, max }) => (
                <div key={key} className="space-y-1">
                  <div className="flex justify-between text-xs">
                    <span>{label}</span>
                    <span>{gain[key].toFixed(2)} / {max.toFixed(1)}</span>
                  </div>
                  <div className="flex items-center gap-2">
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
                      className="w-20 px-2 py-1 text-xs rounded border border-input bg-background text-right"
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
