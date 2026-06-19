// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { create } from '@bufbuild/protobuf';

import { TuningSlider } from '@/components/dashboard/actuators/tuning-slider';
import {
  clampTuningValue,
  staticLimitsForJoint,
} from '@/data/actuator-joints';
import {
  ActuatorLimitSnapshotSchema,
  JointActuatorLimitSchema,
} from '@/gen/marengo/v1/marengo_pb';
import {
  findSnapshotLimit,
  kpMaxForJoint,
  resolveJointLimits,
} from '@/state/actuatorStore';

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe('clampTuningValue', () => {
  it('clamps above live kp max to the cap', () => {
    expect(clampTuningValue(60, 50)).toBe(50);
  });

  it('preserves values within the cap envelope', () => {
    expect(clampTuningValue(25, 50)).toBe(25);
  });
});

describe('resolveJointLimits', () => {
  it('prefers live snapshot caps over static fallback', () => {
    const snapshot = create(ActuatorLimitSnapshotSchema, {
      timestampMs: 1n,
      joints: [
        create(JointActuatorLimitSchema, {
          joint: 'shoulder_pitch',
          kpMax: 42,
          kdMax: 3,
          velocityMaxRadS: 1.5,
          tauFfMaxNm: 2,
          wired: true,
        }),
      ],
    });

    const limits = resolveJointLimits('left_shoulder_pitch', snapshot);
    expect(limits?.kpMax).toBe(42);
    expect(kpMaxForJoint('left_shoulder_pitch', snapshot)).toBe(42);
  });

  it('falls back to static limits when snapshot cache is empty', () => {
    const staticLimits = staticLimitsForJoint('left_elbow');
    expect(staticLimits).not.toBeNull();
    expect(resolveJointLimits('left_elbow', null)).toEqual(staticLimits);
    expect(findSnapshotLimit(null, 'left_elbow')).toBeNull();
  });
});

describe('TuningSlider debounce', () => {
  it('debounces onChange by ~250ms without a confirm modal', async () => {
    vi.useFakeTimers();
    const onDebouncedChange = vi.fn();

    render(
      <TuningSlider
        label="Runtime kp"
        value={10}
        min={0}
        max={50}
        step={0.5}
        onDebouncedChange={onDebouncedChange}
      />,
    );

    const slider = screen.getByRole('slider', { name: /runtime kp/i });
    fireEvent.change(slider, { target: { value: '30' } });
    expect(onDebouncedChange).not.toHaveBeenCalled();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    vi.advanceTimersByTime(250);
    await Promise.resolve();
    expect(onDebouncedChange).toHaveBeenCalledWith(30);
  });

  it('never sends values above the slider max cap', async () => {
    vi.useFakeTimers();
    const onDebouncedChange = vi.fn();

    render(
      <TuningSlider
        label="Runtime kd"
        value={1}
        min={0}
        max={5}
        step={0.1}
        onDebouncedChange={onDebouncedChange}
      />,
    );

    const slider = screen.getByRole('slider', { name: /runtime kd/i }) as HTMLInputElement;
    fireEvent.change(slider, { target: { value: '9' } });
    expect(Number(slider.value)).toBeLessThanOrEqual(5);

    vi.advanceTimersByTime(250);
    await Promise.resolve();
    expect(onDebouncedChange).toHaveBeenCalledWith(5);
  });
});
