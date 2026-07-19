# consul/src/routes/

## Responsibility

React Router v6 route configuration, lazy-loaded page entry points, root layout with Suspense boundary, and a `RouteHandle` metadata pattern for page header titles and subtitles.

## Design

### Route tree (`config.tsx`)
```
RootLayout (Suspense<Outlet />)
├── / → DashboardPage (lazy)             — robot overview, host cards, charts, URDF preview
├── /simulation → SimulationPage (lazy)  — simulation sessions and scenarios
├── /subsystems → SubsystemsPage (lazy)  — inventory table with live enrichment
├── /logs → LogsPage (lazy)              — structured log viewer with live streaming
└── /testing → TestingPage (lazy)        — MIT motor testing interface
```

- Every child route uses `lazy: () => import(...)` for code splitting. Each page chunk includes its feature overview component, sub-components, and any local hooks/constants.
- No nested route levels — flat under the root layout.

### Root layout (`root-layout.tsx`)
- Single `<Suspense fallback={<PageLoadingFallback />}>` wrapping `<Outlet />`.
- All pages share the `DashboardLayout` chrome (sidebar + header + scroll area) which is imported inside each page component, not in the route definition.

### Route Handle pattern
- Not currently applied in `config.tsx` (the `handle` metadata slot on route objects is not populated), but `lib/route-handle.ts` is wired in `SiteHeader` to walk react-router `useMatches()` and extract `header.title`/`header.subtitle`. This is available for future per-route header customization.

### Index (`index.tsx`)
- Minimal: calls `createBrowserRouter(appRoutes)` and exports `appRouter` for consumption in `main.tsx`.

## Flow

```
main.tsx → <RouterProvider router={appRouter} />
  → react-router resolves URL to route config
  → lazy() fires dynamic import
  → Suspense shows PageLoadingFallback while chunk loads
  → Page component renders DashboardLayout + feature overview
  → SiteHeader calls useMatches() + getRouteHeader() for title
```

## Integration

| Consumer | Route | Database |
|---|---|---|
| `main.tsx` | `appRouter` | All |
| `SiteHeader` | `useMatches()` + `getRouteHeader` | Title/subtitle display |
| `AppProviders` | `RouterProvider` | React tree mounting point |
