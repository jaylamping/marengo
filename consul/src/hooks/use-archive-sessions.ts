import { useEffect, useState } from 'react';

import type { LogsMode } from '@/components/dashboard/logs/logs-mode-tabs';
import { isChappeLive } from '@/lib/chappe-config';
import {
  fetchBenchLines,
  fetchRecentLogs,
  fetchSessions,
  fetchTraceLines,
  type AsyncSlice,
  type LogSessionDto,
} from '@/lib/log-api';
import {
  ensureLogsSeeded,
  hydrateLogsFromSnapshot,
  setLogsPageActive,
  SNAPSHOT_HYDRATE_LIMIT,
} from '@/lib/log-buffer';

export type ArchiveView = 'bench' | 'trace' | 'search';

const emptySessionsSlice = (): AsyncSlice<LogSessionDto[]> => ({
  loading: false,
  error: null,
  data: [],
});

const emptyLinesSlice = (): AsyncSlice<string[]> => ({
  loading: false,
  error: null,
  data: [],
});

export function useArchiveSessions(mode: LogsMode) {
  const [sessionsState, setSessionsState] = useState<AsyncSlice<LogSessionDto[]>>(emptySessionsSlice);
  const [linesState, setLinesState] = useState<AsyncSlice<string[]>>(emptyLinesSlice);
  const [selectedSession, setSelectedSession] = useState<string | null>(null);
  const [archiveView, setArchiveView] = useState<ArchiveView>('bench');

  useEffect(() => {
    setLogsPageActive(true);
    if (isChappeLive()) {
      let cancelled = false;
      void fetchRecentLogs(SNAPSHOT_HYDRATE_LIMIT).then((result) => {
        if (cancelled || !result.ok) {
          return;
        }
        hydrateLogsFromSnapshot(result.data);
      });
      return () => {
        cancelled = true;
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
    setSessionsState((prev) => ({ ...prev, loading: true, error: null }));
    void fetchSessions(50).then((result) => {
      if (result.ok) {
        setSessionsState({ loading: false, error: null, data: result.data });
        return;
      }
      setSessionsState({ loading: false, error: result.error, data: [] });
    });
  }, [mode]);

  useEffect(() => {
    if (mode !== 'archive' || !selectedSession || archiveView === 'search') {
      return;
    }
    setLinesState((prev) => ({ ...prev, loading: true, error: null }));
    const fetchLines =
      archiveView === 'bench'
        ? fetchBenchLines(selectedSession, 0, 500)
        : fetchTraceLines(selectedSession, 0, 500);
    void fetchLines.then((result) => {
      if (result.ok) {
        setLinesState({ loading: false, error: null, data: result.data.lines });
        return;
      }
      setLinesState({ loading: false, error: result.error, data: [] });
    });
  }, [mode, selectedSession, archiveView]);

  return {
    sessionsState,
    linesState,
    selectedSession,
    setSelectedSession,
    archiveView,
    setArchiveView,
  };
}
