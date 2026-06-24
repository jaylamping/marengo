import { useEffect, useState } from 'react';

import type { LogsMode } from '@/components/dashboard/logs/logs-mode-tabs';
import { isChappeLive } from '@/lib/chappe-config';
import {
  fetchBenchLines,
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

export type ArchiveView = 'bench' | 'trace' | 'search';

export function useArchiveSessions(mode: LogsMode) {
  const [sessions, setSessions] = useState<LogSessionDto[]>([]);
  const [selectedSession, setSelectedSession] = useState<string | null>(null);
  const [archiveView, setArchiveView] = useState<ArchiveView>('bench');
  const [archiveLines, setArchiveLines] = useState<string[]>([]);

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

  return {
    sessions,
    selectedSession,
    setSelectedSession,
    archiveView,
    setArchiveView,
    archiveLines,
  };
}
