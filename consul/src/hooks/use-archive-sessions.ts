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

/** Lines fetched per archive session view; `linesTotal` says when there are more. */
export const ARCHIVE_LINES_LIMIT = 500;

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
  const [linesTotal, setLinesTotal] = useState<number | null>(null);
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
    let cancelled = false;
    setSessionsState((prev) => ({ ...prev, loading: true, error: null }));
    void fetchSessions(50).then((result) => {
      if (cancelled) return;
      if (result.ok) {
        setSessionsState({ loading: false, error: null, data: result.data });
        return;
      }
      setSessionsState({ loading: false, error: result.error, data: [] });
    });
    return () => {
      cancelled = true;
    };
  }, [mode]);

  useEffect(() => {
    if (mode !== 'archive' || !selectedSession || archiveView === 'search') {
      return;
    }
    // A response for a session no longer selected is dropped (UI-19).
    let cancelled = false;
    setLinesState((prev) => ({ ...prev, loading: true, error: null }));
    setLinesTotal(null);
    const fetchLines =
      archiveView === 'bench'
        ? fetchBenchLines(selectedSession, 0, ARCHIVE_LINES_LIMIT)
        : fetchTraceLines(selectedSession, 0, ARCHIVE_LINES_LIMIT);
    void fetchLines.then((result) => {
      if (cancelled) return;
      if (result.ok) {
        setLinesState({ loading: false, error: null, data: result.data.lines });
        setLinesTotal(result.data.total);
        return;
      }
      setLinesState({ loading: false, error: result.error, data: [] });
    });
    return () => {
      cancelled = true;
    };
  }, [mode, selectedSession, archiveView]);

  return {
    sessionsState,
    linesState,
    linesTotal,
    selectedSession,
    setSelectedSession,
    archiveView,
    setArchiveView,
  };
}
