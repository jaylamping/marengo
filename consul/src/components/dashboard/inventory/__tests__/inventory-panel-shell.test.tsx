// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';

import {
  inventoryDrawerContentClassName,
  inventoryModalContentClassName,
  inventoryTableShellClassName,
  inventoryToolbarShellClassName,
} from '@/components/dashboard/inventory/constants';
import { InventoryRowModal } from '@/components/dashboard/inventory/inventory-row-modal';
import { isSubsystemInteractive } from '@/components/dashboard/inventory/subsystem-interactive';
import { TooltipProvider } from '@/components/ui/tooltip';
import { InventoryTableToolbar } from '@/components/dashboard/inventory/inventory-table-toolbar';
import { InventoryTableView } from '@/components/dashboard/inventory/inventory-table-view';
import type { InventoryItem } from '@/data/robot-inventory';
import { Tabs } from '@/components/ui/tabs';

vi.mock('@/hooks/use-mobile', () => ({
  useIsMobile: () => false,
}));

vi.mock('@/hooks/use-active-reporting-lease', () => ({
  useActiveReportingLease: () => 'idle',
}));

vi.mock('@/components/dashboard/hardware/hardware-settings-sheet', () => ({
  InventoryLimitsReadOnly: ({ jointName }: { jointName: string }) => (
    <div data-testid="inventory-limits-readonly-stub">Limits stub · {jointName}</div>
  ),
}));

afterEach(() => {
  cleanup();
});

const interactiveActuator: InventoryItem = {
  id: 27,
  name: 'right_upper_arm_yaw',
  group: 'right_arm',
  kind: 'actuator',
  status: 'Enabled',
  value: '0.12',
  limit: '±1.57',
  preset: 'bench_4dof',
  node: 'RS82 · can0 · id 3',
};

const offlineActuator: InventoryItem = {
  id: 24,
  name: 'left_wrist',
  group: 'left_arm',
  kind: 'actuator',
  status: 'Offline',
  value: '—',
  limit: '±1.6',
  preset: 'unassigned',
  node: 'RS00 · can0 · id 18',
};

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

  it('defines modal content panel without row-level blur', () => {
    expect(inventoryModalContentClassName).toContain('bg-surface-1');
    expect(inventoryModalContentClassName).not.toContain('animate-in');
    expect(inventoryDrawerContentClassName).toContain('bg-surface-1');
  });
});

describe('isSubsystemInteractive', () => {
  it('unlocks online configured actuators', () => {
    expect(isSubsystemInteractive(interactiveActuator)).toBe(true);
  });

  it('locks offline or unassigned actuators', () => {
    expect(isSubsystemInteractive(offlineActuator)).toBe(false);
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

describe('InventoryRowModal panel shell', () => {
  it('renders centered modal for the selected device', () => {
    render(
      <TooltipProvider>
        <InventoryRowModal
          item={interactiveActuator}
          items={[offlineActuator, interactiveActuator]}
          open
          onOpenChange={() => undefined}
          onNavigate={() => undefined}
        />
      </TooltipProvider>,
    );

    expect(screen.getByTestId('inventory-row-modal')).toBeTruthy();
    expect(screen.getAllByText('right_upper_arm_yaw').length).toBeGreaterThan(0);
    expect(screen.getByLabelText('Edit name')).toBeTruthy();
    expect(screen.getByLabelText('Edit location')).toBeTruthy();
    expect(screen.getByLabelText('Edit preset')).toBeTruthy();
    expect(
      screen.getAllByText('Enabled').some((el) => el.getAttribute('data-slot') === 'badge'),
    ).toBe(true);
    expect(screen.queryByText('Armed')).toBeNull();
    expect(screen.queryByText('Dummy')).toBeNull();
    expect(screen.queryByRole('heading', { name: 'Identity' })).toBeNull();
  });

  it('dithers actuator command surfaces for offline rows but keeps identity editable', () => {
    render(
      <TooltipProvider>
        <InventoryRowModal
          item={offlineActuator}
          items={[offlineActuator, interactiveActuator]}
          open
          onOpenChange={() => undefined}
          onNavigate={() => undefined}
        />
      </TooltipProvider>,
    );

    expect(
      screen.getAllByText('Offline').some((el) => el.getAttribute('data-slot') === 'badge'),
    ).toBe(true);
    expect(screen.getByLabelText('Edit name')).not.toBeDisabled();
    expect(screen.getByLabelText('Edit location')).toBeTruthy();
    expect(screen.getByLabelText('Edit preset')).toBeTruthy();
    expect(screen.queryByRole('heading', { name: 'Identity' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Home' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Start sweep' })).toBeNull();
    expect(screen.getByTestId('inventory-limits-readonly-stub')).toBeTruthy();
  });

  it('navigates prev/next within the provided list', () => {
    const onNavigate = vi.fn();

    render(
      <TooltipProvider>
        <InventoryRowModal
          item={offlineActuator}
          items={[offlineActuator, interactiveActuator]}
          open
          onOpenChange={() => undefined}
          onNavigate={onNavigate}
        />
      </TooltipProvider>,
    );

    expect(screen.getByText('1 / 2')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Previous subsystem' })).toBeDisabled();

    fireEvent.click(screen.getByRole('button', { name: 'Next subsystem' }));
    expect(onNavigate).toHaveBeenCalledWith(interactiveActuator);
  });

  it('shows actuator telemetry and read-only limits without commissioning actions', () => {
    render(
      <TooltipProvider>
        <InventoryRowModal
          item={interactiveActuator}
          items={[interactiveActuator]}
          open
          onOpenChange={() => undefined}
          onNavigate={() => undefined}
        />
      </TooltipProvider>,
    );

    const modal = screen.getByTestId('inventory-row-modal');
    expect(within(modal).getByText('Telemetry')).toBeTruthy();
    expect(within(modal).queryByText('Tests')).toBeNull();
    expect(within(modal).queryByRole('button', { name: 'Home' })).toBeNull();
    expect(within(modal).queryByRole('button', { name: 'Go' })).toBeNull();
    expect(within(modal).getByTestId('inventory-limits-readonly-stub')).toBeTruthy();
  });
});
