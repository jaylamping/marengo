import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { SidebarUpdateStatus } from '@/components/dashboard/sidebar/sidebar-update-status';
import {
  SELF_UPDATE_TIMEOUT_MS,
  clearSelfUpdateSession,
  writeSelfUpdateSession,
} from '@/lib/version-api';

const toastInfo = vi.fn();
const toastMessage = vi.fn();
const toastError = vi.fn();
const toastSuccess = vi.fn();

vi.mock('sonner', () => ({
  toast: {
    info: (...args: unknown[]) => toastInfo(...args),
    message: (...args: unknown[]) => toastMessage(...args),
    error: (...args: unknown[]) => toastError(...args),
    success: (...args: unknown[]) => toastSuccess(...args),
  },
}));

const fetchVersionStatus = vi.fn();
const startSelfDeploy = vi.fn();

vi.mock('@/lib/version-api', async () => {
  const actual = await vi.importActual<typeof import('@/lib/version-api')>(
    '@/lib/version-api',
  );
  return {
    ...actual,
    fetchVersionStatus: (...args: unknown[]) => fetchVersionStatus(...args),
    startSelfDeploy: (...args: unknown[]) => startSelfDeploy(...args),
  };
});

function status(partial: Record<string, unknown>) {
  return {
    deploy_sha: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
    upstream_sha: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
    upstream_ok: true,
    update_available: true,
    ready_for_target: false,
    ui_state: 'stale',
    deploy: {
      state: 'idle',
      job_id: '',
      target_sha: '',
      result_sha: '',
      unit_name: '',
      started_at: '',
      updated_at: '',
      message: '',
      phase: 'init',
    },
    ...partial,
  };
}

const previewUser = {
  name: 'Joey',
  context: 'live',
  avatar: '',
};

describe('SidebarUpdateStatus', () => {
  beforeEach(() => {
    clearSelfUpdateSession();
    toastInfo.mockReset();
    toastMessage.mockReset();
    toastError.mockReset();
    toastSuccess.mockReset();
    fetchVersionStatus.mockReset();
    startSelfDeploy.mockReset();
    fetchVersionStatus.mockResolvedValue(
      status({
        update_available: false,
        ui_state: 'current',
        upstream_sha: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
      }),
    );
  });

  afterEach(() => {
    cleanup();
  });

  it('shows Check for updates and toasts already current', async () => {
    render(<SidebarUpdateStatus user={previewUser} />);
    await waitFor(() => expect(fetchVersionStatus).toHaveBeenCalled());
    fetchVersionStatus.mockResolvedValueOnce(
      status({
        update_available: false,
        ui_state: 'current',
        upstream_sha: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
      }),
    );
    fireEvent.click(screen.getByTestId('check-for-updates'));
    await waitFor(() => expect(toastInfo).toHaveBeenCalledWith('Already up to date'));
  });

  it('shows Update next to identity when stale and opens confirm dialog', async () => {
    fetchVersionStatus.mockResolvedValue(
      status({
        update_available: true,
        ui_state: 'stale',
        upstream_sha: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
      }),
    );
    render(<SidebarUpdateStatus user={previewUser} />);
    await waitFor(() => expect(screen.getByTestId('sidebar-update-button')).toBeTruthy());
    expect(screen.getByText('Joey')).toBeTruthy();
    fireEvent.click(screen.getByTestId('sidebar-update-button'));
    expect(screen.getByTestId('update-confirm-dialog')).toBeTruthy();
  });

  it('shows Update when last job failed so retry is possible', async () => {
    fetchVersionStatus.mockResolvedValue(
      status({
        update_available: false,
        ui_state: 'failed',
        deploy: {
          state: 'failed',
          job_id: 'j1',
          target_sha: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
          result_sha: '',
          unit_name: 'marengo-self-update',
          started_at: '2026-08-11T00:00:00Z',
          updated_at: '2026-08-11T00:01:00Z',
          message: 'www missing',
          phase: 'error',
        },
      }),
    );
    render(<SidebarUpdateStatus user={previewUser} />);
    await waitFor(() => expect(screen.getByTestId('sidebar-update-button')).toBeTruthy());
  });

  it('shows spinner while deploy job is running', async () => {
    fetchVersionStatus.mockResolvedValue(
      status({
        update_available: true,
        ui_state: 'updating',
        deploy: {
          state: 'running',
          job_id: 'j1',
          target_sha: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
          result_sha: '',
          unit_name: 'marengo-self-update',
          started_at: '2026-08-11T00:00:00Z',
          updated_at: '2026-08-11T00:00:00Z',
          message: 'building',
          phase: 'build',
        },
      }),
    );
    render(<SidebarUpdateStatus user={previewUser} />);
    await waitFor(() => expect(screen.getByTestId('sidebar-update-spinner')).toBeTruthy());
    expect(screen.queryByTestId('sidebar-update-button')).toBeNull();
    expect(screen.queryByTestId('check-for-updates')).toBeNull();
  });

  it('watched job failure paints failed not behind after session clears', async () => {
    writeSelfUpdateSession({
      jobId: 'j-fail',
      startedAtMs: Date.now(),
    });
    fetchVersionStatus.mockResolvedValue(
      status({
        update_available: true,
        ui_state: 'stale',
        deploy: {
          state: 'failed',
          job_id: 'j-fail',
          target_sha: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
          result_sha: '',
          unit_name: 'marengo-self-update',
          started_at: '2026-08-11T00:00:00Z',
          updated_at: '2026-08-11T00:01:00Z',
          message: 'systemd-run failed',
          phase: 'error',
        },
      }),
    );
    render(<SidebarUpdateStatus user={previewUser} />);
    await waitFor(() =>
      expect(screen.getByText(/· failed$/)).toBeTruthy(),
    );
    expect(screen.queryByText(/· behind$/)).toBeNull();
    await waitFor(() =>
      expect(screen.getByTestId('sidebar-update-button')).toBeTruthy(),
    );
    expect(toastError).toHaveBeenCalledWith('systemd-run failed');
  });

  it('times out a watched job even when version status is null', async () => {
    writeSelfUpdateSession({
      jobId: 'j-hang',
      startedAtMs: Date.now() - SELF_UPDATE_TIMEOUT_MS - 1_000,
    });
    fetchVersionStatus.mockResolvedValue(null);
    render(<SidebarUpdateStatus user={previewUser} />);
    await waitFor(() =>
      expect(toastError).toHaveBeenCalledWith('Update timed out'),
    );
    await waitFor(() =>
      expect(screen.queryByTestId('sidebar-update-spinner')).toBeNull(),
    );
    expect(screen.queryByText(/^Updating/)).toBeNull();
  });
});
