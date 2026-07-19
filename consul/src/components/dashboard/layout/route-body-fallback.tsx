import { Skeleton } from '@/components/ui/skeleton';

/** Lightweight placeholder while a route body chunk loads. */
export function RouteBodyFallback() {
  return (
    <div
      className="flex flex-1 flex-col gap-4 px-4 py-6 lg:px-6"
      data-testid="route-body-fallback"
    >
      <Skeleton className="h-14 w-full" />
      <Skeleton className="h-10 w-64" />
      <Skeleton className="min-h-64 w-full flex-1" />
    </div>
  );
}
