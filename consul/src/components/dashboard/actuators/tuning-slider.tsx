import { useEffect, useMemo, useState } from 'react';

import { Label } from '@/components/ui/label';
import { clampTuningValue } from '@/data/actuator-joints';
import { debounceTrailing } from '@/lib/throttle-callback';

export const TUNING_DEBOUNCE_MS = 250;

type TuningSliderProps = {
  label: string;
  value: number;
  min?: number;
  max: number;
  step?: number;
  onDebouncedChange: (value: number) => void;
};

export function TuningSlider({
  label,
  value,
  min = 0,
  max,
  step = 0.1,
  onDebouncedChange,
}: TuningSliderProps) {
  const [localValue, setLocalValue] = useState(value);

  useEffect(() => {
    setLocalValue(clampTuningValue(value, max, min));
  }, [value, max, min]);

  const debouncedEmit = useMemo(
    () => debounceTrailing(onDebouncedChange, TUNING_DEBOUNCE_MS),
    [onDebouncedChange],
  );

  const handleChange = (nextRaw: number) => {
    const clamped = clampTuningValue(nextRaw, max, min);
    setLocalValue(clamped);
    debouncedEmit(clamped);
  };

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-baseline justify-between gap-2 text-sm">
        <Label htmlFor={`tuning-${label}`}>{label}</Label>
        <span className="font-mono tabular-nums text-muted-foreground">
          {localValue.toFixed(2)}
        </span>
      </div>
      <input
        id={`tuning-${label}`}
        type="range"
        role="slider"
        aria-label={label}
        min={min}
        max={max}
        step={step}
        value={localValue}
        onChange={(event) => handleChange(Number(event.target.value))}
        className="w-full accent-primary"
      />
    </div>
  );
}
