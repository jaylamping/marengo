import type { RouteObject } from 'react-router-dom';

import { RootLayout } from '@/routes/root-layout';

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
        path: '/memory',
        lazy: async () => {
          const { MemoryPage } = await import('@/pages/memory');
          return {
            Component: MemoryPage,
            handle: {
              header: {
                title: 'Memory',
                subtitle: 'mem0 · marengo-joey · read-only',
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
