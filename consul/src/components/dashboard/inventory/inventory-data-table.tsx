import { lazy, Suspense, useCallback, useMemo, useState } from 'react';

import type { InventoryItem } from '@/data/robot-inventory';

import { useInventoryTable } from '@/components/dashboard/inventory/hooks/use-inventory-table';
import { InventoryDetailProvider } from '@/components/dashboard/inventory/inventory-detail-context';
import type { InventoryIdentityPatch } from '@/components/dashboard/inventory/inventory-row-modal';
import { InventoryTableFooter } from '@/components/dashboard/inventory/inventory-table-footer';
import { InventoryTableToolbar } from '@/components/dashboard/inventory/inventory-table-toolbar';
import { InventoryTableView } from '@/components/dashboard/inventory/inventory-table-view';
import type { InventoryRow } from '@/components/dashboard/inventory/types';
import { dashboardPanelPointerClassName } from '@/components/dashboard/layout/constants';
import { Tabs } from '@/components/ui/tabs';

const InventoryRowModal = lazy(async () => {
  const module = await import(
    '@/components/dashboard/inventory/inventory-row-modal'
  );
  return { default: module.InventoryRowModal };
});

type InventoryDataTableProps = {
  data: InventoryItem[];
};

type FieldOverrides = Record<number, Partial<InventoryIdentityPatch>>;

function applyOverrides(
  item: InventoryItem,
  overrides: FieldOverrides,
): InventoryRow {
  const patch = overrides[item.id];
  return patch === undefined ? item : { ...item, ...patch };
}

export function InventoryDataTable({ data }: InventoryDataTableProps) {
  const [detailItemId, setDetailItemId] = useState<number | null>(null);
  const [fieldOverrides, setFieldOverrides] = useState<FieldOverrides>({});

  const mergedData = useMemo(
    () => data.map((item) => applyOverrides(item, fieldOverrides)),
    [data, fieldOverrides],
  );

  const {
    activeView,
    setActiveView,
    collapsedGroups,
    data: inventoryData,
    expandedGroupCount,
    groupedSections,
    table,
    toggleGroup,
    expandAllGroups,
    collapseAllGroups,
    viewCounts,
  } = useInventoryTable(mergedData);

  const navigationItems = useMemo(
    () =>
      groupedSections.flatMap((section) =>
        section.rows.map((row) => row.original),
      ),
    [groupedSections],
  );

  const detailItem = useMemo(() => {
    if (detailItemId === null) {
      return null;
    }
    return navigationItems.find((row) => row.id === detailItemId) ?? null;
  }, [detailItemId, navigationItems]);

  const openItem = useCallback((item: InventoryRow) => {
    setDetailItemId(item.id);
  }, []);

  const applyPatch = useCallback((itemId: number, patch: InventoryIdentityPatch) => {
    setFieldOverrides((previous) => ({
      ...previous,
      [itemId]: patch,
    }));
  }, []);

  return (
    <InventoryDetailProvider openItem={openItem}>
      <Tabs
        value={activeView}
        onValueChange={(value) => setActiveView(value as typeof activeView)}
        className={`w-full flex-col justify-start gap-6 ${dashboardPanelPointerClassName}`}
      >
        <InventoryTableToolbar
          activeView={activeView}
          onViewChange={setActiveView}
          viewCounts={viewCounts}
          table={table}
          onExpandAll={expandAllGroups}
          onCollapseAll={collapseAllGroups}
        />
        <div className="relative flex flex-col gap-4 overflow-auto">
          <InventoryTableView
            table={table}
            groupedSections={groupedSections}
            collapsedGroups={collapsedGroups}
            onToggleGroup={toggleGroup}
          />
          <InventoryTableFooter
            table={table}
            groupCount={groupedSections.length}
            expandedGroupCount={expandedGroupCount}
            totalDeviceCount={inventoryData.length}
          />
        </div>
      </Tabs>
      {detailItem ? (
        <Suspense fallback={null}>
          <InventoryRowModal
            item={detailItem}
            items={navigationItems}
            open
            onOpenChange={(open) => {
              if (!open) {
                setDetailItemId(null);
              }
            }}
            onNavigate={(next) => setDetailItemId(next.id)}
            onApply={applyPatch}
          />
        </Suspense>
      ) : null}
    </InventoryDetailProvider>
  );
}
