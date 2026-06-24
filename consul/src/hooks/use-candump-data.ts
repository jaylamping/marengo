import { useEffect, useState } from 'react';

import type { LogsMode } from '@/components/dashboard/logs/logs-mode-tabs';
import { fetchCandumpPage, fetchCandumpSummary, type CandumpFrameDto } from '@/lib/log-api';

export const CAN_PAGE = 200;

export function useCandumpData(mode: LogsMode, selectedSession: string | null) {
  const [canFrames, setCanFrames] = useState<CandumpFrameDto[]>([]);
  const [canTotal, setCanTotal] = useState(0);
  const [canOffset, setCanOffset] = useState(0);
  const [canSummary, setCanSummary] = useState<string>('');

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

  return {
    canFrames,
    canTotal,
    canOffset,
    setCanOffset,
    canSummary,
  };
}
