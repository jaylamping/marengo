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

describe('PresetCell assign path', () => {
  beforeEach(() => {
    localStorage.clear();
    useInventoryOverridesStore.setState({ overrides: {} });
  });

  it('persists the chosen preset through the inventory overrides store', () => {
    render(<PresetCell itemId={20} preset="unassigned" />);

    fireEvent.click(screen.getByRole('button', { name: 'Assign preset' }));
    fireEvent.click(screen.getByRole('button', { name: 'pick bench_4dof' }));

    expect(useInventoryOverridesStore.getState().overrides[20]).toEqual({
      preset: 'bench_4dof',
    });
    expect(localStorage.getItem(INVENTORY_OVERRIDES_STORAGE_KEY)).toContain('bench_4dof');
  });
});
