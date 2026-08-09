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

  it('persists catalog preset tags through the inventory overrides store only', () => {
    render(
      <PresetCell itemId={20} preset="unassigned" jointName="right_elbow_pitch" />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Assign preset tag' }));
    fireEvent.click(screen.getByRole('button', { name: 'pick golden_pose' }));

    expect(useInventoryOverridesStore.getState().overrides[20]).toEqual({
      preset: 'golden_pose',
    });
    expect(localStorage.getItem(INVENTORY_OVERRIDES_STORAGE_KEY)).toContain(
      'golden_pose',
    );
  });
});
