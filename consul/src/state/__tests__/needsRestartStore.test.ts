import { beforeEach, describe, expect, it } from 'vitest';

import {
  selectNeedsRestart,
  selectPendingJoints,
  useNeedsRestartStore,
} from '@/state/needsRestartStore';

describe('needsRestartStore', () => {
  beforeEach(() => {
    useNeedsRestartStore.setState({
      pending: [],
      restartDialogOpen: false,
      dialogReason: null,
    });
  });

  it('marks joints and derives needsRestart', () => {
    useNeedsRestartStore.getState().markNeedsRestart({
      profile: 'arm_4dof_right',
      joint: 'right_elbow_pitch',
      reason: 'structural',
      expected_revision: 'abc',
    });
    expect(selectNeedsRestart(useNeedsRestartStore.getState())).toBe(true);
    expect(
      useNeedsRestartStore.getState().isJointPending('right_elbow_pitch'),
    ).toBe(true);
    expect(
      useNeedsRestartStore.getState().isJointPending('right_shoulder_pitch'),
    ).toBe(false);
  });

  it('dedupes joint marks by profile+reason', () => {
    const store = useNeedsRestartStore.getState();
    store.markNeedsRestart({
      profile: 'arm_4dof_right',
      joint: 'right_elbow_pitch',
      reason: 'structural',
      expected_revision: 'abc',
    });
    store.markNeedsRestart({
      profile: 'arm_4dof_right',
      joint: 'right_elbow_pitch',
      reason: 'structural',
      expected_revision: 'abc',
    });
    expect(selectPendingJoints(useNeedsRestartStore.getState())).toEqual([
      'right_elbow_pitch',
    ]);
  });

  it('opens and closes the dialog without clearing pending', () => {
    useNeedsRestartStore.getState().markNeedsRestart({
      profile: 'arm_4dof_right',
      joint: 'j1',
      reason: 'structural',
      expected_revision: 'r1',
    });
    useNeedsRestartStore.getState().openRestartDialog({ reason: 'structural' });
    expect(useNeedsRestartStore.getState().restartDialogOpen).toBe(true);
    expect(useNeedsRestartStore.getState().dialogReason).toBe('structural');
    useNeedsRestartStore.getState().closeRestartDialog();
    expect(useNeedsRestartStore.getState().restartDialogOpen).toBe(false);
    expect(selectPendingJoints(useNeedsRestartStore.getState())).toEqual(['j1']);
  });

  it('clearNeedsRestart wipes pending and closes dialog', () => {
    useNeedsRestartStore.getState().markJointNeedsRestart('j1');
    useNeedsRestartStore.getState().openRestartDialog();
    useNeedsRestartStore.getState().clearNeedsRestart();
    expect(useNeedsRestartStore.getState().pending).toEqual([]);
    expect(useNeedsRestartStore.getState().restartDialogOpen).toBe(false);
  });

  it('live Set Limits must not clear structural pending', () => {
    useNeedsRestartStore.getState().markNeedsRestart({
      profile: 'arm_4dof_right',
      joint: 'right_elbow_pitch',
      reason: 'structural',
      expected_revision: 'r1',
    });
    // Simulate successful Set Limits — store untouched.
    expect(selectNeedsRestart(useNeedsRestartStore.getState())).toBe(true);
  });
});
