import type { RouteObject } from 'react-router-dom';

import { isActuatorsFeatureEnabled } from '@/lib/feature-flags';
import { RootLayout } from '@/routes/root-layout';

/** Handles stay on the route object so headers work even when Overview is soft-cached. */
const actuatorsRoute: RouteObject = {
  path: '/actuators',
  handle: {
    header: {
      title: 'Actuators',
      subtitle: 'telemetry only · PR-1 shell',
    },
  },
  lazy: async () => {
    const { ActuatorsPage } = await import('@/pages/actuators');
    return { Component: ActuatorsPage };
  },
};

export const appRoutes: RouteObject[] = [
  {
    element: <RootLayout />,
    children: [
      {
        path: '/',
        handle: {
          header: {
            title: 'Overview',
            subtitle: 'arm_2dof_right · bench',
          },
        },
        lazy: async () => {
          const { DashboardPage } = await import('@/pages/dashboard');
          return { Component: DashboardPage };
        },
      },
      {
        path: '/simulation',
        handle: {
          header: {
            title: 'Simulation',
            subtitle: 'Isaac Sim · Isaac Lab · wireframe',
          },
        },
        lazy: async () => {
          const { SimulationPage } = await import('@/pages/simulation');
          return { Component: SimulationPage };
        },
      },
      ...(isActuatorsFeatureEnabled() ? [actuatorsRoute] : []),
      {
        path: '/subsystems',
        handle: {
          header: {
            title: 'Subsystems',
            subtitle: 'devices · actuators · sensors',
          },
        },
        lazy: async () => {
          const { SubsystemsPage } = await import('@/pages/subsystems');
          return { Component: SubsystemsPage };
        },
      },
      {
        path: '/logs',
        handle: {
          header: {
            title: 'Logs',
            subtitle: 'live stream · virtualized',
          },
        },
        lazy: async () => {
          const { LogsPage } = await import('@/pages/logs');
          return { Component: LogsPage };
        },
      },
      {
        path: '/testing',
        handle: {
          header: {
            title: 'Testing',
            subtitle: 'interactive · multi-actuator · PID retune',
          },
        },
        lazy: async () => {
          const { TestingPage } = await import('@/pages/testing');
          return { Component: TestingPage };
        },
      },
    ],
  },
];
