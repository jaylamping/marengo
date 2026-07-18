import type { RouteObject } from 'react-router-dom';

import { isActuatorsFeatureEnabled } from '@/lib/feature-flags';
import { RootLayout } from '@/routes/root-layout';

const actuatorsRoute: RouteObject = {
  path: '/actuators',
  lazy: async () => {
    const { ActuatorsPage } = await import('@/pages/actuators');
    return {
      Component: ActuatorsPage,
      handle: {
        header: {
          title: 'Actuators',
          subtitle: 'telemetry only · PR-1 shell',
        },
      },
    };
  },
};

export const appRoutes: RouteObject[] = [
  {
    element: <RootLayout />,
    children: [
      {
        path: '/',
        lazy: async () => {
          const { DashboardPage } = await import('@/pages/dashboard');
          return {
            Component: DashboardPage,
            handle: {
              header: {
                title: 'Overview',
                subtitle: 'marengo_arm_4dof · bench',
              },
            },
          };
        },
      },
      {
        path: '/simulation',
        lazy: async () => {
          const { SimulationPage } = await import('@/pages/simulation');
          return {
            Component: SimulationPage,
            handle: {
              header: {
                title: 'Simulation',
                subtitle: 'Isaac Sim · Isaac Lab · wireframe',
              },
            },
          };
        },
      },
      ...(isActuatorsFeatureEnabled() ? [actuatorsRoute] : []),
      {
        path: '/subsystems',
        lazy: async () => {
          const { SubsystemsPage } = await import('@/pages/subsystems');
          return {
            Component: SubsystemsPage,
            handle: {
              header: {
                title: 'Subsystems',
                subtitle: 'devices · actuators · sensors',
              },
            },
          };
        },
      },
      {
        path: '/logs',
        lazy: async () => {
          const { LogsPage } = await import('@/pages/logs');
          return {
            Component: LogsPage,
            handle: {
              header: {
                title: 'Logs',
                subtitle: 'live stream · virtualized',
              },
            },
          };
        },
      },
      {
        path: '/testing',
        lazy: async () => {
          const { TestingPage } = await import('@/pages/testing');
          return {
            Component: TestingPage,
            handle: {
              header: {
                title: 'Testing',
                subtitle: 'interactive · multi-actuator · PID retune',
              },
            },
          };
        },
      },
    ],
  },
];
