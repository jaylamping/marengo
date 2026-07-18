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
          data: { frames: result.data.frames, total: result.data.total_frames },
        });
        return;
      }
      setPageState({
        loading: false,
        error: result.error,
        data: { frames: [], total: 0 },
      });
    });

    if (selectedSession) {
      setSummaryState((prev) => ({ ...prev, loading: true, error: null }));
      void fetchCandumpSummary(selectedSession).then((result) => {
        if (result.ok) {
          const summary = result.data;
          setSummaryState({
            loading: false,
            error: null,
            data: `${summary.frame_count} frames · ${summary.approx_hz.toFixed(1)} Hz · ${summary.duration_s.toFixed(2)}s`,
          });
          return;
        }
        setSummaryState({ loading: false, error: result.error, data: '' });
      });
    } else {
      setSummaryState({ loading: false, error: null, data: 'live candump-latest.log' });
    }
  }, [mode, selectedSession, canOffset]);

  return {
    pageState,
    summaryState,
    canOffset,
    setCanOffset,
  };
}
