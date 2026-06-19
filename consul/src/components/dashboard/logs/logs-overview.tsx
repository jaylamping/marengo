import { useEffect, useState } from 'react';

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
import {
  fetchBenchLines,
  fetchCandumpPage,
  fetchCandumpSummary,
  fetchRecentLogs,
  fetchSessions,
  fetchTraceLines,
  type LogSessionDto,
} from '@/lib/log-api';
import {
  ensureLogsSeeded,
  hydrateLogsFromSnapshot,
  setLogsPageActive,
  SNAPSHOT_HYDRATE_LIMIT,
} from '@/lib/log-buffer';
import { isChappeLive } from '@/lib/chappe-config';
import type { LogEntry } from '@/data/logs';

const CAN_PAGE = 200;
type ArchiveView = 'bench' | 'trace' | 'search';

function LogsOverviewInner() {
  const [mode, setMode] = useState<LogsMode>('live');
  const [archiveView, setArchiveView] = useState<ArchiveView>('bench');
  const [sessions, setSessions] = useState<LogSessionDto[]>([]);
  const [selectedSession, setSelectedSession] = useState<string | null>(null);
  const [archiveLines, setArchiveLines] = useState<string[]>([]);
  const [canFrames, setCanFrames] = useState<Awaited<ReturnType<typeof fetchCandumpPage>>['frames']>([]);
  const [canTotal, setCanTotal] = useState(0);
  const [canOffset, setCanOffset] = useState(0);
  const [canSummary, setCanSummary] = useState<string>('');
  const [autoFollow, setAutoFollow] = useState(true);
  const [selectedLog, setSelectedLog] = useState<LogEntry | null>(null);
  const [detailOpen, setDetailOpen] = useState(false);

  function handleSelectLog(entry: LogEntry) {
    setSelectedLog(entry);
    setDetailOpen(true);
  }

  useEffect(() => {
    setLogsPageActive(true);
    if (isChappeLive()) {
      void fetchRecentLogs(SNAPSHOT_HYDRATE_LIMIT).then((entries) => {
        hydrateLogsFromSnapshot(entries);
      });
      return () => {
        setLogsPageActive(false);
      };
    }
    ensureLogsSeeded();
    return () => {
      setLogsPageActive(false);
    };
  }, []);

  useEffect(() => {
    if (mode !== 'archive' && mode !== 'can') {
      return;
    }
    void fetchSessions(50).then(setSessions);
  }, [mode]);

  useEffect(() => {
    if (mode !== 'archive' || !selectedSession) {
      return;
    }
    if (archiveView === 'bench') {
      void fetchBenchLines(selectedSession, 0, 500).then(({ lines }) => setArchiveLines(lines));
    } else {
      void fetchTraceLines(selectedSession, 0, 500).then(({ lines }) => setArchiveLines(lines));
    }
  }, [mode, selectedSession, archiveView]);

  useEffect(() => {
    if (mode !== 'can') {
      return;
    }
    const id = selectedSession ?? 'latest';
    void fetchCandumpPage(id, canOffset, CAN_PAGE).then(({ frames, total_frames }) => {
      setCanFrames(frames);
      setCanTotal(total_frames);
    });
    if (selectedSession) {
      void fetchCandumpSummary(selectedSession).then((summary) => {
        if (!summary) {
          setCanSummary('');
          return;
        }
        setCanSummary(
          `${summary.frame_count} frames · ${summary.approx_hz.toFixed(1)} Hz · ${summary.duration_s.toFixed(2)}s`,
        );
      });
    } else {
      setCanSummary('live candump-latest.log');
    }
  }, [mode, selectedSession, canOffset]);

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
