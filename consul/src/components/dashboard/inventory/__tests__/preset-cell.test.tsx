// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReactNode } from 'react';

import { PresetCell } from '@/components/dashboard/inventory/cells/preset-cell';
import {
  INVENTORY_OVERRIDES_STORAGE_KEY,
  useInventoryOverridesStore,
} from '@/state/inventoryOverridesStore';

vi.mock('@/lib/config-api', () => ({
  applyActuatorConfig: vi.fn(async () => ({
    ok: true,
    message: 'preview',
    applied_live: false,
    restart_required: false,
    persist_status: 'n/a',
    decision: 'noop',
    revision: 'abc',
  })),
}));

vi.mock('@/lib/query-client', () => ({
  queryClient: { invalidateQueries: vi.fn(async () => undefined) },
}));

vi.mock('sonner', () => ({
  toast: {
    message: vi.fn(),
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('@/components/ui/select', () => {
  return {
    Select: ({
      onValueChange,
      children,
    }: {
      onValueChange?: (value: string) => void;
      children?: ReactNode;
    }) => (
      <div data-testid="preset-select-stub">
        <button type="button" onClick={() => onValueChange?.('golden_pose')}>
          pick golden_pose
        </button>
        <button type="button" onClick={() => onValueChange?.('bench_4dof')}>
          pick bench_4dof
        </button>
        {children}
      </div>
    ),
    SelectTrigger: ({ children }: { children?: ReactNode }) => <div>{children}</div>,
    SelectValue: () => null,
    SelectContent: ({ children }: { children?: ReactNode }) => <div>{children}</div>,
    SelectGroup: ({ children }: { children?: ReactNode }) => <div>{children}</div>,
    SelectItem: ({ children }: { children?: ReactNode }) => <div>{children}</div>,
  };
});

afterEach(() => {
  cleanup();
});

describe('PresetCell', () => {
  beforeEach(() => {
    localStorage.clear();
    useInventoryOverridesStore.setState({ overrides: {} });
  });

  it('persists unmapped catalog presets through the inventory overrides store', () => {
    render(
      <PresetCell itemId={20} preset="unassigned" jointName="right_elbow_pitch" />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Assign preset' }));
    fireEvent.click(screen.getByRole('button', { name: 'pick golden_pose' }));

    expect(useInventoryOverridesStore.getState().overrides[20]).toEqual({
      preset: 'golden_pose',
    });
    expect(localStorage.getItem(INVENTORY_OVERRIDES_STORAGE_KEY)).toContain(
      'golden_pose',
    );
  });

  it('routes mapped bench presets through the apply API instead of localStorage', async () => {
    const { applyActuatorConfig } = await import('@/lib/config-api');
    render(
      <PresetCell
        itemId={25}
        preset="bench_3dof"
        jointName="right_shoulder_pitch"
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Edit preset bench_3dof' }));
    fireEvent.click(screen.getByRole('button', { name: 'pick bench_4dof' }));

    await vi.waitFor(() => {
      expect(applyActuatorConfig).toHaveBeenCalled();
    });
    expect(useInventoryOverridesStore.getState().overrides[25]).toBeUndefined();
  });
});
