/** Map live nav URLs to their lazy page modules for hover/idle prefetch. */
const ROUTE_PREFETCHERS: Record<string, () => Promise<unknown>> = {
  '/': () => import('@/pages/dashboard'),
  '/testing': () => import('@/pages/testing'),
  '/logs': () => import('@/pages/logs'),
  '/simulation': () => import('@/pages/simulation'),
  '/subsystems': () => import('@/pages/subsystems'),
  '/actuators': () => import('@/pages/actuators'),
};

/** Heavy bodies split behind thin route shells. */
const BODY_PREFETCHERS: Record<string, () => Promise<unknown>> = {
  '/testing': () => import('@/components/dashboard/testing/testing-overview'),
  '/logs': () => import('@/components/dashboard/logs/logs-overview'),
  '/simulation': () => import('@/components/dashboard/simulation/simulation-overview'),
  '/subsystems': async () => {
    await import('@/components/dashboard/subsystems/subsystems-overview');
    // Warm inventory graph on hover only — never in eager prefetchHeavyRoutes.
    await import('@/components/dashboard/inventory/inventory-data-table');
  },
};

const warmed = new Set<string>();

/** Prefetch a route's page chunk (no-op if unknown or already warmed). */
export function prefetchRoute(url: string): void {
  if (warmed.has(url)) {
    return;
  }
  const load = ROUTE_PREFETCHERS[url];
  if (!load) {
    return;
  }
  warmed.add(url);
  void load();
  const body = BODY_PREFETCHERS[url];
  if (body) {
    void body();
  }
}

/**
 * Warm common destinations after first paint.
 * Intentionally skips inventory-data-table — that graph is huge and should
 * only load when the user actually opens Subsystems (or hovers it).
 */
export function prefetchHeavyRoutes(): void {
  prefetchRoute('/testing');
  prefetchRoute('/logs');
  prefetchRoute('/simulation');
}
