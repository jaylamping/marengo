import { useEffect, useState } from 'react';

import type { LogsMode } from '@/components/dashboard/logs/logs-mode-tabs';
import {
  fetchCandumpPage,
  fetchCandumpSummary,
  type AsyncSlice,
  type CandumpFrameDto,
} from '@/lib/log-api';

export const CAN_PAGE = 200;

type CandumpPageData = {
  frames: CandumpFrameDto[];
  total: number;
};

const emptyPageSlice = (): AsyncSlice<CandumpPageData> => ({
  loading: false,
  error: null,
  data: { frames: [], total: 0 },
});

const emptySummarySlice = (): AsyncSlice<string> => ({
  loading: false,
  error: null,
  data: '',
});

function formatSummaryLine(summary: {
  parsed_frames: number;
  total_lines: number;
  approx_hz: number | null;
  duration_s: number;
}): string {
  const hz =
    summary.approx_hz == null ? 'n/a' : `${summary.approx_hz.toFixed(1)} Hz`;
  return `${summary.parsed_frames} frames (${summary.total_lines} lines) · ${hz} · ${summary.duration_s.toFixed(2)}s`;
}

export function useCandumpData(mode: LogsMode, selectedSession: string | null) {
  const [pageState, setPageState] = useState<AsyncSlice<CandumpPageData>>(emptyPageSlice);
  const [summaryState, setSummaryState] = useState<AsyncSlice<string>>(emptySummarySlice);
  const [canOffset, setCanOffset] = useState(0);

  useEffect(() => {
    if (mode !== 'can') {
      return;
    }
    const id = selectedSession ?? 'latest';
    setPageState((prev) => ({ ...prev, loading: true, error: null }));
    void fetchCandumpPage(id, canOffset, CAN_PAGE).then((result) => {
      if (result.ok) {
        setPageState({
          loading: false,
          error: null,
          data: {
            frames: result.data.frames,
            total: result.data.parsed_frames ?? result.data.total_frames,
          },
        });
        return;
      }
      setPageState({
        loading: false,
        error: result.error,
        data: { frames: [], total: 0 },
      });
    });

    setSummaryState((prev) => ({ ...prev, loading: true, error: null }));
    void fetchCandumpSummary(id).then((result) => {
      if (result.ok) {
        setSummaryState({
          loading: false,
          error: null,
          data: formatSummaryLine(result.data),
        });
        return;
      }
      setSummaryState({ loading: false, error: result.error, data: '' });
    });
  }, [mode, selectedSession, canOffset]);

  return {
    pageState,
    summaryState,
    canOffset,
    setCanOffset,
  };
}
