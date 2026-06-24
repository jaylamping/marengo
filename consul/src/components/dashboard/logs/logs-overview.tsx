import { useState } from 'react';

import { dashboardLogsClassName } from '@/components/dashboard/layout/constants';
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
import { useArchiveSessions, type ArchiveView } from '@/hooks/use-archive-sessions';
import { useCandumpData, CAN_PAGE } from '@/hooks/use-candump-data';
import { useLogDetailSheet } from '@/hooks/use-log-detail-sheet';

function LogsOverviewInner() {
  const [mode, setMode] = useState<LogsMode>('live');
  const [autoFollow, setAutoFollow] = useState(true);

  const {
    sessions,
    selectedSession,
    setSelectedSession,
    archiveView,
    setArchiveView,
    archiveLines,
  } = useArchiveSessions(mode);

  const {
    canFrames,
    canTotal,
    canOffset,
    setCanOffset,
    canSummary,
  } = useCandumpData(mode, selectedSession);

  const {
    selectedLog,
    detailOpen,
    setDetailOpen,
    handleSelectLog,
  } = useLogDetailSheet();

  return (
    <div className={dashboardLogsClassName}>
      <div className="flex flex-col gap-3">
        <LogsConnectionBanner />
        <LogsModeTabs mode={mode} onModeChange={setMode} />
      </div>

      {mode === 'live' ? (
        <LogsFilterProvider>
          <LogsToolbar autoFollow={autoFollow} onAutoFollowChange={setAutoFollow} />
          <LogsVirtualTable
            autoFollow={autoFollow && mode === 'live'}
            selectedLogId={selectedLog?.id ?? null}
            onSelectLog={handleSelectLog}
          />
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
            sessions={sessions}
            selectedId={selectedSession}
            onSelect={setSelectedSession}
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
            <div className={logsArchivePanelShellClassName}>
              {archiveView === 'search' ? (
                <LogsArchiveSearch
                  selectedLogId={selectedLog?.id ?? null}
                  onSelectLog={handleSelectLog}
                />
              ) : archiveLines.length === 0 ? (
                <p className="text-sm text-muted-foreground">Select a session.</p>
              ) : (
                <VirtualLinesList lines={archiveLines} emptyMessage="Select a session." />
              )}
            </div>
          </div>
        </div>
      ) : null}

      {mode === 'can' ? (
        <div className="grid min-h-0 flex-1 gap-4 lg:grid-cols-[240px_1fr]">
          <LogsSessionList
            sessions={sessions}
            selectedId={selectedSession}
            onSelect={(id) => {
              setSelectedSession(id);
              setCanOffset(0);
            }}
          />
          <div className="flex min-h-0 flex-col gap-2">
            <p className="text-sm text-muted-foreground">{canSummary}</p>
            <CandumpFrameTable
              frames={canFrames}
              total={canTotal}
              offset={canOffset}
              pageSize={CAN_PAGE}
              onPage={setCanOffset}
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
