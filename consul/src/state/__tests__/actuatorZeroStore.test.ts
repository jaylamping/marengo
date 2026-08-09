// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { isReferenceReady } from '@/lib/commissioning';
import { useActuatorZeroStore } from '@/state/actuatorZeroStore';

const STORAGE_KEY = 'marengo.consul.actuatorZeroed.v1';

describe('actuatorZeroStore (no readiness persistence)', () => {
  beforeEach(() => {
    window.localStorage.clear();
    useActuatorZeroStore.getState().reset();
  });

  afterEach(() => {
    window.localStorage.clear();
    useActuatorZeroStore.getState().reset();
    vi.restoreAllMocks();
  });

  it('does not write readiness marks to localStorage', () => {
    useActuatorZeroStore.getState().markZeroed('right_shoulder_pitch');
    useActuatorZeroStore
      .getState()
      .markAllZeroed(['right_shoulder_roll', 'right_elbow_pitch']);
    expect(window.localStorage.getItem(STORAGE_KEY)).toBeNull();
  });

  it('ignores stale localStorage readiness on load', async () => {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ right_shoulder_pitch: true }),
    );
    // Re-import store module would keep singleton; assert Reference never follows storage.
    expect(useActuatorZeroStore.getState().isZeroed('right_shoulder_pitch')).toBe(
      false,
    );
    expect(isReferenceReady(undefined)).toBe(false);
  });

  it('markZeroed is a no-op for commissioning readiness', () => {
    useActuatorZeroStore.getState().markZeroed('right_shoulder_pitch');
    expect(useActuatorZeroStore.getState().isZeroed('right_shoulder_pitch')).toBe(
      false,
    );
  });
});
