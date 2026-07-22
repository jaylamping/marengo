import { beforeEach, describe, expect, it } from 'vitest';

import {
  selectNeedsRestart,
  useNeedsRestartStore,
} from '@/state/needsRestartStore';

describe('needsRestartStore', () => {
  beforeEach(() => {
    useNeedsRestartStore.setState({
      pendingRestartJoints: [],
      restartDialogOpen: false,
      dialogFromApply: false,
    });
  });

  it('marks joints and derives needsRestart', () => {
    useNeedsRestartStore.getState().markJointNeedsRestart('right_elbow_pitch');
    expect(selectNeedsRestart(useNeedsRestartStore.getState())).toBe(true);
    expect(
      useNeedsRestartStore.getState().isJointPending('right_elbow_pitch'),
    ).toBe(true);
    expect(
      useNeedsRestartStore.getState().isJointPending('right_shoulder_pitch'),
    ).toBe(false);
  });

  it('dedupes joint marks', () => {
    const store = useNeedsRestartStore.getState();
    store.markJointNeedsRestart('right_elbow_pitch');
    store.markJointNeedsRestart('right_elbow_pitch');
    expect(useNeedsRestartStore.getState().pendingRestartJoints).toEqual([
      'right_elbow_pitch',
    ]);
  });

  it('opens and closes the dialog without clearing pending', () => {
    useNeedsRestartStore.getState().markJointNeedsRestart('j1');
    useNeedsRestartStore.getState().openRestartDialog({ fromApply: true });
    expect(useNeedsRestartStore.getState().restartDialogOpen).toBe(true);
    expect(useNeedsRestartStore.getState().dialogFromApply).toBe(true);
    useNeedsRestartStore.getState().closeRestartDialog();
    expect(useNeedsRestartStore.getState().restartDialogOpen).toBe(false);
    expect(useNeedsRestartStore.getState().pendingRestartJoints).toEqual(['j1']);
  });

  it('clearNeedsRestart wipes pending and closes dialog', () => {
    useNeedsRestartStore.getState().markJointNeedsRestart('j1');
    useNeedsRestartStore.getState().openRestartDialog();
    useNeedsRestartStore.getState().clearNeedsRestart();
    expect(useNeedsRestartStore.getState().pendingRestartJoints).toEqual([]);
    expect(useNeedsRestartStore.getState().restartDialogOpen).toBe(false);
  });
});
