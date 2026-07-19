/** Map live nav URLs to their lazy page modules for hover/idle prefetch. */
const ROUTE_PREFETCHERS: Record<string, () => Promise<unknown>> = {
  '/': () => import('@/pages/dashboard'),
  '/testing': () => import('@/pages/testing'),
  '/logs': () => import('@/pages/logs'),
  '/simulation': () => import('@/pages/simulation'),
  '/subsystems': () => import('@/pages/subsystems'),
  '/actuators': () => import('@/pages/actuators'),
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
}

/** Warm the routes that usually hitch on first click after Overview. */
export function prefetchHeavyRoutes(): void {
  prefetchRoute('/testing');
  prefetchRoute('/logs');
}
