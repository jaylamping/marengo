import { useState } from 'react';

import { dashboardLogsClassName } from '@/components/dashboard/layout/constants';
import { DeferredMount } from '@/components/dashboard/layout/deferred-mount';
import { logsArchivePanelShellClassName, logsTabsVariant } from '@/components/dashboard/logs/constants';
import { CandumpFrameTable } from '@/components/dashboard/logs/candump-frame-table';
import { VirtualLinesList } from '@/components/dashboard/logs/virtual-lines-list';
import { LogDetailSheet } from '@/components/dashboard/logs/log-detail-sheet';
import { LogsArchiveSearch } from '@/components/dashboard/logs/logs-archive-search';
import { LogsConnectionBanner } from '@/components/dashboard/logs/logs-connection-banner';
import { LogsFilterProvider } from '@/components/dashboard/logs/logs-filter-context';
import { LogsModeTabs, type LogsMode } from '@/components/dashboard/logs/logs-mode-tabs';
import { LogsSessionList } from '@/components/dashboard/logs/logs-session-list';
import { LogsToolbar } from '@/components/dashboard/logs/logs-toolbar';
import { LogsVirtualTable } from '@/components/dashboard/logs/logs-virtual-table';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Skeleton } from '@/components/ui/skeleton';
import { useArchiveSessions, type ArchiveView } from '@/hooks/use-archive-sessions';
import { useCandumpData, CAN_PAGE } from '@/hooks/use-candump-data';
import { useLogDetailSheet } from '@/hooks/use-log-detail-sheet';
import { logErrorMessage, shouldShowLogErrorBanner } from '@/lib/log-api';

function LogsOverviewInner() {
  const [mode, setMode] = useState<LogsMode>('live');
  const [autoFollow, setAutoFollow] = useState(true);

  const {
    sessionsState,
    linesState,
    selectedSession,
    setSelectedSession,
    archiveView,
    setArchiveView,
  } = useArchiveSessions(mode);

  const { pageState, summaryState, canOffset, setCanOffset } = useCandumpData(mode, selectedSession);

  const {
    selectedLog,
    detailOpen,
    setDetailOpen,
    handleSelectLog,
  } = useLogDetailSheet();

  const renderArchivePanel = () => {
    if (archiveView === 'search') {
      return (
        <LogsArchiveSearch
          selectedLogId={selectedLog?.id ?? null}
          onSelectLog={handleSelectLog}
        />
      );
    }

    if (shouldShowLogErrorBanner(linesState.error)) {
      return (
        <p className="p-4 text-sm text-destructive">{logErrorMessage(linesState.error!)}</p>
      );
    }

    if (linesState.loading) {
      return <p className="p-4 text-sm text-muted-foreground">Loading archive lines…</p>;
    }

    if (!selectedSession) {
      return <p className="p-4 text-sm text-muted-foreground">Select a session.</p>;
    }

    if (linesState.data.length === 0) {
      return <p className="p-4 text-sm text-muted-foreground">No lines for this session.</p>;
    }

    return <VirtualLinesList lines={linesState.data} emptyMessage="Select a session." />;
  };

  return (
    <div className={dashboardLogsClassName}>
      <div className="flex flex-col gap-3">
        <LogsConnectionBanner />
        <LogsModeTabs mode={mode} onModeChange={setMode} />
      </div>

      {mode === 'live' ? (
        <LogsFilterProvider>
          <LogsToolbar autoFollow={autoFollow} onAutoFollowChange={setAutoFollow} />
          <DeferredMount
            fallback={<Skeleton className="min-h-0 flex-1 w-full rounded-[4px]" />}
            timeoutMs={120}
          >
            <LogsVirtualTable
              autoFollow={autoFollow && mode === 'live'}
              selectedLogId={selectedLog?.id ?? null}
              onSelectLog={handleSelectLog}
            />
          </DeferredMount>
        </LogsFilterProvider>
      ) : null}

      <LogDetailSheet
        entry={selectedLog}
        open={detailOpen}
        onOpenChange={setDetailOpen}
      />

      {mode === 'archive' ? (
        <div className="grid min-h-0 flex-1 gap-4 lg:grid-cols-[240px_1fr]">
          <LogsSessionList
            sessions={sessionsState.data}
            selectedId={selectedSession}
            onSelect={setSelectedSession}
            error={sessionsState.error}
            loading={sessionsState.loading}
          />
          <div className="flex min-h-0 flex-col gap-2">
            <Tabs value={archiveView} onValueChange={(v) => setArchiveView(v as ArchiveView)}>
              <TabsList variant={logsTabsVariant}>
                <TabsTrigger variant={logsTabsVariant} value="bench">
                  Bench log
                </TabsTrigger>
                <TabsTrigger variant={logsTabsVariant} value="trace">
                  Position trace
                </TabsTrigger>
                <TabsTrigger variant={logsTabsVariant} value="search">
                  Search
                </TabsTrigger>
              </TabsList>
            </Tabs>
            <div className={logsArchivePanelShellClassName}>{renderArchivePanel()}</div>
          </div>
        </div>
      ) : null}

      {mode === 'can' ? (
        <div className="grid min-h-0 flex-1 gap-4 lg:grid-cols-[240px_1fr]">
          <LogsSessionList
            sessions={sessionsState.data}
            selectedId={selectedSession}
            onSelect={(id) => {
              setSelectedSession(id);
              setCanOffset(0);
            }}
            error={sessionsState.error}
            loading={sessionsState.loading}
          />
          <div className="flex min-h-0 flex-col gap-2">
            {shouldShowLogErrorBanner(summaryState.error) ? (
              <p className="text-sm text-destructive">{logErrorMessage(summaryState.error!)}</p>
            ) : (
              <p className="text-sm text-muted-foreground">{summaryState.data}</p>
            )}
            <CandumpFrameTable
              frames={pageState.data.frames}
              total={pageState.data.total}
              offset={canOffset}
              pageSize={CAN_PAGE}
              onPage={setCanOffset}
              error={pageState.error}
              loading={pageState.loading}
            />
          </div>
        </div>
      ) : null}
    </div>
  );
}

export function LogsOverview() {
  return <LogsOverviewInner />;
}
