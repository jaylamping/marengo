import { InventoryDataTable } from '@/components/dashboard/inventory/inventory-data-table';
import { dashboardSubsystemsClassName } from '@/components/dashboard/layout/constants';
import { useEnrichedInventory } from '@/hooks/use-enriched-inventory';
import { useLiveInventory } from '@/hooks/use-live-inventory';

/**
 * Read-only live master inventory for `/telemetry`.
 * Commissioning mutations live on Hardware only.
 * (Page-level DeferredLazyBody owns deferral; keep the table eager here.)
 */
export function TelemetryOverview() {
  const { data: enriched } = useEnrichedInventory();
  const inventory = useLiveInventory(enriched);

  return (
    <div
      className={dashboardSubsystemsClassName}
      data-testid="telemetry-overview"
    >
      <p className="font-mono text-[11px] tracking-wide text-muted-foreground">
        Live read-only master inventory · calibration and Enable on{' '}
        <span className="text-foreground">Hardware</span>
      </p>
      <InventoryDataTable data={inventory} />
    </div>
  );
}
