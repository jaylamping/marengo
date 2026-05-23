import { createBrowserRouter, type RouteObject } from 'react-router-dom';

import { DashboardPage } from '@/pages/dashboard';
import { SimulationPage } from '@/pages/simulation';

export const appRoutes: RouteObject[] = [
  {
    path: '/',
    element: <DashboardPage />,
    handle: {
      header: {
        title: 'Overview',
        subtitle: 'marengo_arm_4dof · bench',
      },
    },
  },
  {
    path: '/simulation',
    element: <SimulationPage />,
    handle: {
      header: {
        title: 'Simulation',
        subtitle: 'Isaac Sim · Isaac Lab · wireframe',
      },
    },
  },
];

export const appRouter = createBrowserRouter(appRoutes);
