import { lazy, Suspense, useCallback, useMemo, useState } from 'react';

import type { InventoryItem } from '@/data/robot-inventory';

import { useInventoryTable } from '@/components/dashboard/inventory/hooks/use-inventory-table';
import { InventoryDetailProvider } from '@/components/dashboard/inventory/inventory-detail-context';
import { InventoryTableFooter } from '@/components/dashboard/inventory/inventory-table-footer';
import { InventoryTableToolbar } from '@/components/dashboard/inventory/inventory-table-toolbar';
import { InventoryTableView } from '@/components/dashboard/inventory/inventory-table-view';
import type { InventoryRow } from '@/components/dashboard/inventory/types';
import { dashboardPanelPointerClassName } from '@/components/dashboard/layout/constants';
import { Tabs } from '@/components/ui/tabs';

const InventoryRowDrawer = lazy(async () => {
  const module = await import(
    '@/components/dashboard/inventory/inventory-row-drawer'
  );
  return { default: module.InventoryRowDrawer };
});

type InventoryDataTableProps = {
  data: InventoryItem[];
};

export function InventoryDataTable({ data }: InventoryDataTableProps) {
  const [detailItem, setDetailItem] = useState<InventoryRow | null>(null);
  const [limitOverrides, setLimitOverrides] = useState<Record<number, string>>(
    {},
  );

  const mergedData = useMemo(
    () =>
      data.map((item) => {
        const override = limitOverrides[item.id];
        return override === undefined ? item : { ...item, limit: override };
      }),
    [data, limitOverrides],
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

  const openItem = useCallback(
    (item: InventoryRow) => {
      const override = limitOverrides[item.id];
      setDetailItem(
        override === undefined ? item : { ...item, limit: override },
      );
    },
    [limitOverrides],
  );

  const applyLimit = useCallback((itemId: number, limit: string) => {
    setLimitOverrides((previous) => ({ ...previous, [itemId]: limit }));
    setDetailItem((previous) =>
      previous && previous.id === itemId ? { ...previous, limit } : previous,
    );
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
          <InventoryRowDrawer
            item={detailItem}
            open
            onOpenChange={(open) => {
              if (!open) {
                setDetailItem(null);
              }
            }}
            onApplyLimit={applyLimit}
          />
        </Suspense>
      ) : null}
    </InventoryDetailProvider>
  );
}
