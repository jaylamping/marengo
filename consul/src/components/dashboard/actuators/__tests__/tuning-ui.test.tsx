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
  jointLimitMax,
  liveJointLimits,
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
  it('prefers live snapshot caps over static display reference', () => {
    const snapshot = create(ActuatorLimitSnapshotSchema, {
      timestampMs: 1n,
      joints: [
        create(JointActuatorLimitSchema, {
          joint: 'right_shoulder_pitch',
          kpMax: 42,
          kdMax: 3,
          velocityMaxRadS: 1.5,
          tauFfMaxNm: 2,
          wired: true,
        }),
      ],
    });

    const limits = resolveJointLimits('right_shoulder_pitch', snapshot);
    expect(limits?.kpMax).toBe(42);
    expect(jointLimitMax('right_shoulder_pitch', snapshot, 'kp')).toBe(42);
    expect(liveJointLimits('right_shoulder_pitch', snapshot)?.kdMax).toBe(3);
  });

  it('does not arm live command caps from static display limits', () => {
    const staticLimits = staticLimitsForJoint('right_shoulder_roll');
    expect(staticLimits).not.toBeNull();
    expect(liveJointLimits('right_shoulder_roll', null)).toBeNull();
    expect(jointLimitMax('right_shoulder_roll', null, 'kp')).toBeNull();
    expect(resolveJointLimits('right_shoulder_roll', null)).toEqual(staticLimits);
    expect(findSnapshotLimit(null, 'right_shoulder_roll')).toBeNull();
  });
});

describe('TuningSlider debounce', () => {
  it('debounces onChange by ~250ms without a confirm modal', async () => {
    vi.useFakeTimers();
    const onDebouncedChange = vi.fn();

    render(
      <TuningSlider
        label="Runtime kp"
        value={0}
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
        value={0}
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
