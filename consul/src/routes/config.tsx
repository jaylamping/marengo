import type { RouteObject } from 'react-router-dom';

import { isActuatorsFeatureEnabled } from '@/lib/feature-flags';
import { ActuatorsPage } from '@/pages/actuators';
import { DashboardPage } from '@/pages/dashboard';
import { HardwarePage } from '@/pages/hardware';
import { LogsPage } from '@/pages/logs';
import { SimulationPage } from '@/pages/simulation';
import { SubsystemsPage } from '@/pages/subsystems';
import { TestingPage } from '@/pages/testing';
import { RootLayout } from '@/routes/root-layout';

/**
 * Thin page shells are static imports so location updates immediately.
 * Heavy bodies stay behind React.lazy + DeferredMount inside each page.
 * (React Router route.lazy blocks pathname until the module resolves — that was the hang.)
 */
const actuatorsRoute: RouteObject = {
  path: '/actuators',
  Component: ActuatorsPage,
  handle: {
    header: {
      title: 'Actuators',
      subtitle: 'telemetry only · PR-1 shell',
    },
  },
};

export const appRoutes: RouteObject[] = [
  {
    element: <RootLayout />,
    children: [
      {
        path: '/',
        Component: DashboardPage,
        handle: {
          header: {
            title: 'Overview',
            subtitle: 'arm_4dof_right · bench',
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
      ...(isActuatorsFeatureEnabled() ? [actuatorsRoute] : []),
      {
        path: '/subsystems',
        Component: SubsystemsPage,
        handle: {
          header: {
            title: 'Subsystems',
            subtitle: 'devices · actuators · sensors',
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
