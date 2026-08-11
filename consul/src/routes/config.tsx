import type { RouteObject } from 'react-router-dom';

import { DashboardPage } from '@/pages/dashboard';
import { HardwarePage } from '@/pages/hardware';
import { LogsPage } from '@/pages/logs';
import { SimulationPage } from '@/pages/simulation';
import { SubsystemsPage } from '@/pages/subsystems';
import { TelemetryPage } from '@/pages/telemetry';
import { TestingPage } from '@/pages/testing';
import { SidebarUpdatePreviewPage } from '@/pages/sidebar-update-preview';
import { RootLayout } from '@/routes/root-layout';

/**
 * Thin page shells are static imports so location updates immediately.
 * Heavy bodies stay behind React.lazy + DeferredMount inside each page.
 * (React Router route.lazy blocks pathname until the module resolves — that was the hang.)
 */
export const appRoutes: RouteObject[] = [
  ...(import.meta.env.DEV
    ? [
        {
          path: '/dev/sidebar-update-preview',
          Component: SidebarUpdatePreviewPage,
        } satisfies RouteObject,
      ]
    : []),
  {
    element: <RootLayout />,
    children: [
      {
        path: '/',
        Component: DashboardPage,
        handle: {
          header: {
            title: 'Overview',
            subtitle: 'master inventory · robot.yaml',
          },
        },
      },
      {
        path: '/simulation',
        Component: SimulationPage,
        handle: {
          header: {
            title: 'Simulation',
            subtitle: 'Isaac Sim · Isaac Lab · wireframe',
          },
        },
      },
      {
        path: '/telemetry',
        Component: TelemetryPage,
        handle: {
          header: {
            title: 'Telemetry',
            subtitle: 'live · read-only',
          },
        },
      },
      {
        path: '/subsystems',
        Component: SubsystemsPage,
        handle: {
          header: {
            title: 'Telemetry',
            subtitle: 'live · read-only',
          },
        },
      },
      {
        path: '/hardware',
        Component: HardwarePage,
        handle: {
          header: {
            title: 'Hardware',
            subtitle: 'master config · URDF · Set Limits',
          },
        },
      },
      {
        path: '/logs',
        Component: LogsPage,
        handle: {
          header: {
            title: 'Logs',
            subtitle: 'live stream · virtualized',
          },
        },
      },
      {
        path: '/testing',
        Component: TestingPage,
        handle: {
          header: {
            title: 'Testing',
            subtitle: 'interactive · multi-actuator · PID retune',
          },
        },
      },
    ],
  },
];
