# consul/src/routes/

## Responsibility

React Router v6 route configuration, lazy-loaded page entry points, root layout with Suspense boundary, and a `RouteHandle` metadata pattern for page header titles and subtitles.

## Design

### Route tree (`config.tsx`)
```
RootLayout (Suspense<Outlet />)
├── / → DashboardPage                   — overview cards, CAN bus spectrum
├── /simulation → SimulationPage        — simulation sessions and scenarios
├── /telemetry → TelemetryPage          — read-only live joint table
├── /subsystems → SubsystemsPage        — legacy URL, redirects to /telemetry
├── /hardware → HardwarePage            — master config, URDF import, Set Limits, Set Zero
├── /logs → LogsPage                    — structured log viewer with live streaming
└── /testing → TestingPage              — hold-at, PID retune, compound playback
(dev only) /dev/sidebar-update-preview → SidebarUpdatePreviewPage
```

- Page shells are static imports, because a route-level `lazy()` blocked the pathname update until the chunk loaded. Each shell lazy-loads its overview body with `React.lazy` inside `DeferredLazyBody`.
- No nested route levels — flat under the root layout.

### Root layout (`root-layout.tsx`)
- Single `<Suspense fallback={<PageLoadingFallback />}>` wrapping `<Outlet />`.
- All pages share the `DashboardLayout` chrome (sidebar + header + scroll area) which is imported inside each page component, not in the route definition.

### Route Handle pattern
- Every route in `config.tsx` sets `handle.header` (title and subtitle). `SiteHeader` walks react-router `useMatches()` through `lib/route-handle.ts` to show them.

### Index (`index.tsx`)
- Minimal: calls `createBrowserRouter(appRoutes)` and exports `appRouter` for consumption in `main.tsx`.

## Flow

```
main.tsx → <RouterProvider router={appRouter} />
  → react-router resolves URL to route config
  → the static page shell renders at once
  → React.lazy loads the overview body; the shell shows a skeleton meanwhile
  → SiteHeader calls useMatches() + getRouteHeader() for title
```

## Integration

| Consumer | Route | Database |
|---|---|---|
| `main.tsx` | `appRouter` | All |
| `SiteHeader` | `useMatches()` + `getRouteHeader` | Title/subtitle display |
| `AppProviders` | `RouterProvider` | React tree mounting point |
