import { DashboardPage } from '@/pages/dashboard';

export const appRoutes = [
  {
    path: '/',
    element: <DashboardPage />,
  },
] as const;
