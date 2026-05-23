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
    ],
  },
];
