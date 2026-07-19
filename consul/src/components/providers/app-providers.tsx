import type { ReactNode } from 'react';
import { ThemeProvider } from 'next-themes';
import { QueryClientProvider } from '@tanstack/react-query';
import { PersistQueryClientProvider } from '@tanstack/react-query-persist-client';

import { Toaster } from '@/components/ui/sonner';
import { TooltipProvider } from '@/components/ui/tooltip';
import { persistOptions, queryClient } from '@/lib/query-client';
import { useChappeTelemetry } from '@/hooks/use-chappe-telemetry';

type AppProvidersProps = {
  children: ReactNode;
};

function ChappeTelemetryBridge() {
  useChappeTelemetry();
  return null;
}

function AppTree({ children }: AppProvidersProps) {
  return (
    <ThemeProvider attribute="class" defaultTheme="dark" enableSystem={false}>
      <TooltipProvider>
        <ChappeTelemetryBridge />
        {children}
        <Toaster />
      </TooltipProvider>
    </ThemeProvider>
  );
}

export function AppProviders({ children }: AppProvidersProps) {
  if (persistOptions) {
    return (
      <PersistQueryClientProvider client={queryClient} persistOptions={persistOptions}>
        <AppTree>{children}</AppTree>
      </PersistQueryClientProvider>
    );
  }

  return (
    <QueryClientProvider client={queryClient}>
      <AppTree>{children}</AppTree>
    </QueryClientProvider>
  );
}
