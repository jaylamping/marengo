import { Skeleton } from '@/components/ui/skeleton';

/** Compact fallback for the main pane — chrome and SceneBackground stay mounted. */
export function PageLoadingFallback() {
  return (
    <div
      className="flex flex-1 items-center justify-center px-4 py-12"
      data-testid="page-loading-fallback"
    >
      <div className="w-full max-w-sm space-y-3">
        <Skeleton className="h-8 w-3/4" />
        <Skeleton className="h-4 w-1/2" />
        <Skeleton className="h-32 w-full rounded-[4px]" />
      </div>
    </div>
  );
}
