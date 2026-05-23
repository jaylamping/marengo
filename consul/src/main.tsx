import React from 'react';
import ReactDOM from 'react-dom/client';
import { RouterProvider } from 'react-router-dom';

import { AppProviders } from '@/components/providers/app-providers';
import { appRouter } from '@/routes';

import './index.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AppProviders>
      <RouterProvider router={appRouter} />
    </AppProviders>
  </React.StrictMode>,
);
