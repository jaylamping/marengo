// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { CommissioningScopeEditor } from '@/components/dashboard/hardware/commissioning-scope-editor';
import {
  deleteCommissioningScope,
  fetchCommissioningScope,
  putCommissioningScope,
} from '@/lib/gateway-api';

vi.mock('@/lib/gateway-api', () => ({
  fetchCommissioningScope: vi.fn(),
  putCommissioningScope: vi.fn(),
  deleteCommissioningScope: vi.fn(),
}));

vi.mock('@/lib/config-api', () => ({
  fetchConfigSnapshot: vi.fn(async () => ({
    profile: 'master',
    config_dir: '/opt/marengo/config',
    joints: [
      'right_shoulder_roll',
      'right_shoulder_pitch',
      'right_upper_arm_yaw',
      'right_elbow_pitch',
    ],
    motors: [],
    control_limits: [],
  })),
}));

function renderEditor() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <CommissioningScopeEditor />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('CommissioningScopeEditor', () => {
  it('loads effective scope and applies a narrowed joint list', async () => {
    vi.mocked(fetchCommissioningScope).mockResolvedValue({
      version: 1,
      joints: ['right_shoulder_roll', 'right_shoulder_pitch'],
      ceiling: null,
      effective: ['right_shoulder_roll', 'right_shoulder_pitch'],
      persisted: true,
    });
    vi.mocked(putCommissioningScope).mockResolvedValue({
      version: 1,
      joints: ['right_shoulder_roll'],
      ceiling: null,
      effective: ['right_shoulder_roll'],
      persisted: true,
    });

    renderEditor();
    expect(await screen.findByTestId('commissioning-scope-editor')).toBeTruthy();
    expect(await screen.findByTestId('scope-effective')).toHaveTextContent(
      'right_shoulder_roll',
    );

    const input = screen.getByTestId('scope-joints-input') as HTMLTextAreaElement;
    fireEvent.change(input, { target: { value: 'right_shoulder_roll' } });
    fireEvent.click(screen.getByTestId('scope-apply-btn'));

    await waitFor(() => {
      expect(vi.mocked(putCommissioningScope).mock.calls[0]?.[0]).toEqual({
        joints: ['right_shoulder_roll'],
        confirm_widen: false,
      });
    });
  });

  it('requires confirm_widen when effective scope grows', async () => {
    vi.mocked(fetchCommissioningScope).mockResolvedValue({
      version: 1,
      joints: ['right_shoulder_roll'],
      ceiling: null,
      effective: ['right_shoulder_roll'],
      persisted: true,
    });
    vi.mocked(putCommissioningScope).mockResolvedValue({
      version: 1,
      joints: ['right_shoulder_roll', 'right_shoulder_pitch'],
      ceiling: null,
      effective: ['right_shoulder_roll', 'right_shoulder_pitch'],
      persisted: true,
    });

    renderEditor();
    await screen.findByTestId('commissioning-scope-editor');

    const input = screen.getByTestId('scope-joints-input') as HTMLTextAreaElement;
    fireEvent.change(input, {
      target: { value: 'right_shoulder_roll\nright_shoulder_pitch' },
    });
    fireEvent.click(screen.getByTestId('scope-apply-btn'));

    expect(await screen.findByTestId('scope-widen-confirm')).toBeTruthy();
    fireEvent.click(screen.getByTestId('scope-widen-confirm-btn'));

    await waitFor(() => {
      expect(vi.mocked(putCommissioningScope).mock.calls[0]?.[0]).toEqual({
        joints: ['right_shoulder_roll', 'right_shoulder_pitch'],
        confirm_widen: true,
      });
    });
  });

  it('clears persisted scope via DELETE', async () => {
    vi.mocked(fetchCommissioningScope).mockResolvedValue({
      version: 1,
      joints: ['right_shoulder_roll'],
      ceiling: null,
      effective: ['right_shoulder_roll'],
      persisted: true,
    });
    vi.mocked(deleteCommissioningScope).mockResolvedValue({
      version: 1,
      joints: [],
      ceiling: null,
      effective: [],
      persisted: false,
    });

    renderEditor();
    await screen.findByTestId('commissioning-scope-editor');
    fireEvent.click(screen.getByTestId('scope-clear-btn'));

    await waitFor(() => {
      expect(deleteCommissioningScope).toHaveBeenCalled();
    });
  });
});
