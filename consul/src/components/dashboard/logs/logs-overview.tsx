import { useEffect } from 'react';

import { dashboardLogsClassName } from '@/components/dashboard/layout/constants';
import { LogsFilterProvider } from '@/components/dashboard/logs/logs-filter-context';
import { LogsToolbar } from '@/components/dashboard/logs/logs-toolbar';
import { LogsVirtualTable } from '@/components/dashboard/logs/logs-virtual-table';
import { ensureLogsSeeded } from '@/lib/log-buffer';

function LogsOverviewContent() {
  useEffect(() => {
    ensureLogsSeeded();
  }, []);

  return (
    <div className={dashboardLogsClassName}>
      <LogsToolbar />
      <LogsVirtualTable />
    </div>
  );
}

export function LogsOverview() {
  return (
    <LogsFilterProvider>
      <LogsOverviewContent />
    </LogsFilterProvider>
  );
}
