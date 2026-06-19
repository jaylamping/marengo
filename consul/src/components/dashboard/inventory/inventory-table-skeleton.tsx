import { inventoryTableShellClassName } from '@/components/dashboard/inventory/constants';
import { Skeleton } from '@/components/ui/skeleton';

export function InventoryTableSkeleton() {
  return (
    <div className="flex w-full flex-col gap-6 px-4 lg:px-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Skeleton className="h-9 w-72" />
        <Skeleton className="h-9 w-40" />
      </div>
      <div className={inventoryTableShellClassName}>
        <div className="space-y-0 border-b bg-muted/20 p-3">
          <Skeleton className="h-4 w-full max-w-3xl" />
        </div>
        {Array.from({ length: 8 }, (_, index) => (
          <div key={index} className="flex items-center gap-4 border-b p-3 last:border-b-0">
            <Skeleton className="h-4 w-4 shrink-0" />
            <Skeleton className="h-4 w-32" />
            <Skeleton className="h-4 w-24" />
            <Skeleton className="h-4 w-20" />
            <Skeleton className="ml-auto h-4 w-16" />
          </div>
        ))}
      </div>
    </div>
  );
}
