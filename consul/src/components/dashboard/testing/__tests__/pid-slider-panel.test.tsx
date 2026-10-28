// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { create } from '@bufbuild/protobuf';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { PidSliderPanel } from '@/components/dashboard/testing/pid-slider-panel';
import {
  ActuatorLimitSnapshotSchema,
  JointActuatorLimitSchema,
} from '@/gen/marengo/v1/marengo_pb';
import { postTestingMitCommandBatch } from '@/lib/gateway-api';
import { useActuatorStore } from '@/state/actuatorStore';
import { useTestingStore } from '@/state/testingStore';

const JOINT = 'right_elbow_pitch';

vi.mock('@/hooks/use-config-snapshot', () => ({
  useConfigSnapshot: () => ({
    data: { motors: [{ joint: 'right_elbow_pitch', motor_type: 'rs03' }] },
  }),
}));

vi.mock('@/lib/gateway-api', () => ({
  postEnableCommand: vi.fn(),
  postTestingMitCommandBatch: vi.fn(),
}));

beforeEach(() => {
  vi.mocked(postTestingMitCommandBatch).mockReset().mockResolvedValue(undefined);
  useTestingStore.setState({ selectedJointNames: [JOINT], gains: {}, dryRun: false });
  useActuatorStore.getState().setLimitSnapshot(
    create(ActuatorLimitSnapshotSchema, {
      joints: [
        create(JointActuatorLimitSchema, {
          joint: JOINT,
          kpMax: 5000,
          kdMax: 100,
          tauFfMaxNm: 5,
        }),
      ],
    }),
  );
});

afterEach(() => {
  cleanup();
  useActuatorStore.getState().setLimitSnapshot(null);
  useTestingStore.setState({ selectedJointNames: [], gains: {}, dryRun: true });
});

describe('PID slider panel (UI-06, UI-25)', () => {
  it('Fc stops at the live tau_ff cap the Pi clamps to', () => {
    render(<PidSliderPanel />);
    expect(screen.getAllByRole('slider')[3]).toHaveAttribute('max', '5');
    expect(screen.getAllByRole('slider')[0]).toHaveAttribute('max', '500');
  });

  it('a drag sends one batch, on release, not one per tick', () => {
    render(<PidSliderPanel />);
    const kp = screen.getAllByRole('slider')[0];
    for (const value of ['10', '20', '30', '40']) {
      fireEvent.change(kp, { target: { value } });
    }
    expect(postTestingMitCommandBatch).not.toHaveBeenCalled();
    fireEvent.pointerUp(kp);
    expect(postTestingMitCommandBatch).toHaveBeenCalledTimes(1);
    expect(useTestingStore.getState().gains[JOINT]?.kp).toBe(40);
  });
});
