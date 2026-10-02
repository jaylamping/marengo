// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { LocalLimitSyncSession } from '@/components/dashboard/hardware/local-limit-sync-session';
import {
  localLimitSyncCredential,
  setLocalLimitSyncCredential,
} from '@/lib/local-limit-sync-session';

afterEach(() => {
  cleanup();
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
  setLocalLimitSyncCredential('');
});

describe('local mirror session credential', () => {
  it('appears only when the local mirror is configured', () => {
    vi.stubEnv('VITE_LIMIT_SYNC_URL', '');
    render(<LocalLimitSyncSession />);
    expect(screen.queryByLabelText('Local checkout mirror')).toBeNull();
  });

  it('uses runtime input, survives panel remount, and clears without persistent storage', () => {
    vi.stubEnv('VITE_LIMIT_SYNC_URL', 'http://127.0.0.1:8790');
    const localStorageWrite = vi.spyOn(Storage.prototype, 'setItem');
    const view = render(<LocalLimitSyncSession />);
    const input = screen.getByLabelText('Local checkout mirror') as HTMLInputElement;
    expect(input.type).toBe('password');
    fireEvent.change(input, { target: { value: 'isolated-runtime-ui-fixture' } });
    expect(localLimitSyncCredential()).toBe('isolated-runtime-ui-fixture');
    view.unmount();
    render(<LocalLimitSyncSession />);
    const remounted = screen.getByLabelText('Local checkout mirror') as HTMLInputElement;
    expect(remounted.value).toBe('isolated-runtime-ui-fixture');
    fireEvent.change(remounted, { target: { value: '' } });
    expect(localLimitSyncCredential()).toBe('');
    expect(localStorageWrite).not.toHaveBeenCalled();
    localStorageWrite.mockRestore();
  });
});
