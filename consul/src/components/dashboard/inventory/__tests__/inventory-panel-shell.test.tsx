// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import {
  inventoryDrawerContentClassName,
  inventoryTableShellClassName,
  inventoryToolbarShellClassName,
} from '@/components/dashboard/inventory/constants';
import { InventoryRowDrawer } from '@/components/dashboard/inventory/inventory-row-drawer';
import { TooltipProvider } from '@/components/ui/tooltip';
import { InventoryTableToolbar } from '@/components/dashboard/inventory/inventory-table-toolbar';
import { InventoryTableView } from '@/components/dashboard/inventory/inventory-table-view';
import type { InventoryItem } from '@/data/robot-inventory';
import { Tabs } from '@/components/ui/tabs';

vi.mock('@/hooks/use-mobile', () => ({
  useIsMobile: () => false,
}));

afterEach(() => {
  cleanup();
});

describe('inventory panel shell constants (data + chrome tiers)', () => {
  it('defines a data-tier table shell with blur and pointer-events-auto', () => {
    expect(inventoryTableShellClassName).toContain('bg-surface-1');
    expect(inventoryTableShellClassName).toContain('pointer-events-auto');
    expect(inventoryTableShellClassName).toContain('border-line');
  });

  it('defines a chrome-tier toolbar shell with blur and pointer-events-auto', () => {
    expect(inventoryToolbarShellClassName).toContain('bg-surface-1');
    expect(inventoryToolbarShellClassName).toContain('pointer-events-auto');
  });

  it('defines drawer content panel without row-level blur', () => {
    expect(inventoryDrawerContentClassName).toContain('bg-surface-1');
    expect(inventoryDrawerContentClassName).not.toContain('animate-in');
  });
});

describe('InventoryTableView panel shell', () => {
  it('wraps the TanStack table in the data-tier shell', () => {
    const table = {
      getHeaderGroups: () => [
        {
          id: 'header',
          headers: [
            {
              id: 'name',
              colSpan: 1,
              isPlaceholder: false,
              column: { columnDef: { header: 'Name' } },
              getContext: () => ({}),
            },
          ],
        },
      ],
    };

    render(
      <InventoryTableView
        table={table as never}
        groupedSections={[]}
        collapsedGroups={new Set()}
        onToggleGroup={() => undefined}
      />,
    );

    const shell = screen.getByTestId('inventory-table-shell');
    expect(shell.className).toContain('bg-surface-1');
    expect(shell.className).toMatch(/pointer-events-auto/);
    expect(screen.getByText('No devices in this view.')).toBeTruthy();
  });
});

describe('InventoryTableToolbar panel shell', () => {
  it('renders chrome-tier controls inside the toolbar shell', () => {
    render(
      <Tabs value="all">
        <InventoryTableToolbar
          activeView="all"
          onViewChange={() => undefined}
          viewCounts={{ faults: 1, offline: 2, unconfigured: 0 }}
          table={
            {
              getAllColumns: () => [],
            } as never
          }
          onExpandAll={() => undefined}
          onCollapseAll={() => undefined}
        />
      </Tabs>,
    );

    const shell = screen.getByTestId('inventory-toolbar-shell');
    expect(shell.className).toContain('bg-surface-1');
    expect(screen.getByRole('button', { name: 'Expand all' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Collapse all' })).toBeTruthy();
  });
});

describe('InventoryRowDrawer panel shell', () => {
  it('renders controlled drawer content for the selected device', () => {
    const item: InventoryItem = {
      id: 1,
      name: 'shoulder_pitch_r',
      group: 'right_arm',
      kind: 'actuator',
      status: 'Enabled',
      value: '0.12',
      limit: '±1.8',
      preset: 'bench_default',
      node: 'can0:0x01',
    };

    render(
      <TooltipProvider>
        <InventoryRowDrawer item={item} open onOpenChange={() => undefined} />
      </TooltipProvider>,
    );

    expect(screen.getByText('shoulder_pitch_r')).toBeTruthy();
  });
});
