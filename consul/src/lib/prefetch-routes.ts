/** Heavy bodies only — thin route shells are static, so routing never awaits them. */
const BODY_PREFETCHERS: Record<string, () => Promise<unknown>> = {
  '/testing': () => import('@/components/dashboard/testing/testing-overview'),
  '/logs': () => import('@/components/dashboard/logs/logs-overview'),
  '/simulation': () => import('@/components/dashboard/simulation/simulation-overview'),
  '/subsystems': () => import('@/components/dashboard/subsystems/subsystems-overview'),
  '/actuators': () => import('@/components/dashboard/actuators/actuators-overview'),
};

const warmed = new Set<string>();

/** Prefetch a route body on hover (shells are already in the bundle). */
export function prefetchRoute(url: string): void {
  if (warmed.has(url)) {
    return;
  }
  const load = BODY_PREFETCHERS[url];
  if (!load) {
    return;
  }
  warmed.add(url);
  void load();
}

/**
 * Idle-warm common bodies after first paint.
 * Never start these on the click frame — DeferredMount + Suspense own that.
 */
export function prefetchHeavyRoutes(): void {
  const warm = () => {
    prefetchRoute('/testing');
    prefetchRoute('/logs');
    prefetchRoute('/simulation');
  };
  if (typeof requestIdleCallback === 'undefined') {
    window.setTimeout(warm, 2500);
    return;
  }
  requestIdleCallback(warm, { timeout: 5000 });
}
