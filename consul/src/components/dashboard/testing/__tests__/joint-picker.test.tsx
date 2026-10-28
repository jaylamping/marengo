// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { HoldAtControls } from '@/components/dashboard/testing/hold-at-controls';
import { PresetGroupButtons } from '@/components/dashboard/testing/preset-group-buttons';
import { postTestingMitCommandBatch } from '@/lib/gateway-api';
import { useTestingStore } from '@/state/testingStore';

vi.mock('@/hooks/use-config-snapshot', () => ({
  useConfigSnapshot: () => ({
    data: { joints: ['right_shoulder_pitch', 'right_elbow_pitch'] },
  }),
}));

vi.mock('@/lib/gateway-api', () => ({
  postEnableCommand: vi.fn(),
  postTestingMitCommandBatch: vi.fn(),
}));

afterEach(() => {
  cleanup();
  useTestingStore.setState({ selectedJointNames: [], isRunning: false, dryRun: true });
  vi.clearAllMocks();
});

describe('Testing joint selection (UI-13)', () => {
  it('Both Arms selects only the command-eligible joints from config', () => {
    render(<PresetGroupButtons />);
    fireEvent.click(screen.getByRole('button', { name: 'Both Arms' }));
    expect(useTestingStore.getState().selectedJointNames).toEqual([
      'right_shoulder_pitch',
      'right_elbow_pitch',
    ]);
  });

  it('a refused Start Hold shows the error and offers Start again', async () => {
    vi.mocked(postTestingMitCommandBatch).mockRejectedValue(
      new Error('joint not command-eligible: left_elbow'),
    );
    useTestingStore.setState({ selectedJointNames: ['left_elbow'], dryRun: false });
    render(<HoldAtControls />);
    fireEvent.click(screen.getByRole('button', { name: 'Start Hold' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('command-eligible');
    expect(screen.getByRole('button', { name: 'Start Hold' })).toBeInTheDocument();
  });
});
