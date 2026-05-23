import type { InventoryItem } from '@/data/robot-inventory';

import { useInventoryTable } from '@/components/dashboard/inventory/hooks/use-inventory-table';
import { InventoryTableFooter } from '@/components/dashboard/inventory/inventory-table-footer';
import { InventoryTableToolbar } from '@/components/dashboard/inventory/inventory-table-toolbar';
import { InventoryTableView } from '@/components/dashboard/inventory/inventory-table-view';
import { Tabs } from '@/components/ui/tabs';

type InventoryDataTableProps = {
  data: InventoryItem[];
};

export function InventoryDataTable({ data }: InventoryDataTableProps) {
  const {
    activeView,
    setActiveView,
    collapsedGroups,
    data: inventoryData,
    dataIds,
    expandedGroupCount,
    groupedSections,
    handleDragEnd,
    sensors,
    sortableId,
    table,
    toggleGroup,
    expandAllGroups,
    collapseAllGroups,
    viewCounts,
  } = useInventoryTable(data);

  return (
    <Tabs
      value={activeView}
      onValueChange={(value) => setActiveView(value as typeof activeView)}
      className="w-full flex-col justify-start gap-6"
    >
      <InventoryTableToolbar
        activeView={activeView}
        onViewChange={setActiveView}
        viewCounts={viewCounts}
        table={table}
        onExpandAll={expandAllGroups}
        onCollapseAll={collapseAllGroups}
      />
      <div className="relative flex flex-col gap-4 overflow-auto px-4 lg:px-6">
        <InventoryTableView
          table={table}
          groupedSections={groupedSections}
          collapsedGroups={collapsedGroups}
          dataIds={dataIds}
          sortableId={sortableId}
          sensors={sensors}
          onDragEnd={handleDragEnd}
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
  );
}
