import { useEffect, useRef, useState } from 'react';

import {
  SPECTRUM_POLL_MS,
  TAIL_PAGE_LIMIT,
  buildCanTrafficSpectrum,
  readCanLiveChip,
  type CanTrafficSpectrum,
} from '@/lib/can-traffic-spectrum';
import {
  fetchCandumpPage,
  fetchCandumpSummary,
  type CandumpFrameDto,
  type CandumpSummaryDto,
  type LogApiError,
} from '@/lib/log-api';
import { useHostMetricsStore } from '@/state/hostMetricsStore';

export type UseCanTrafficSpectrumArgs = {
  active: boolean;
  sessionId?: string | 'latest';
};

type PollSlices = {
  summary: CandumpSummaryDto | null;
  page: { frames: CandumpFrameDto[]; total: number } | null;
  summaryError: LogApiError | null;
  pageError: LogApiError | null;
};

function idleView(nowMs: number): CanTrafficSpectrum {
  return buildCanTrafficSpectrum({
    summary: null,
    page: null,
    live: readCanLiveChip(null),
    previous: null,
    nowMs,
    summaryError: null,
    pageError: null,
  });
}

export type CanTrafficSpectrumView = CanTrafficSpectrum & {
  loading: boolean;
};

export function useCanTrafficSpectrum({
  active,
  sessionId = 'latest',
}: UseCanTrafficSpectrumArgs): CanTrafficSpectrumView {
  const piMetrics = useHostMetricsStore((s) => s.piMetrics);
  const [spectrum, setSpectrum] = useState<CanTrafficSpectrum>(() => idleView(Date.now()));
  const [loading, setLoading] = useState(true);
  const spectrumRef = useRef(spectrum);
  const loadingRef = useRef(loading);
  spectrumRef.current = spectrum;
  loadingRef.current = loading;

  useEffect(() => {
    if (!active) {
      return;
    }

    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    setLoading(true);

    const poll = async () => {
      const [summaryResult, pageResult] = await Promise.all([
        fetchCandumpSummary(sessionId),
        fetchCandumpPage(sessionId, 0, TAIL_PAGE_LIMIT),
      ]);
      if (cancelled) {
        return;
      }

      const slices: PollSlices = {
        summary: summaryResult.ok ? summaryResult.data : null,
        page: pageResult.ok
          ? {
              frames: pageResult.data.frames,
              total:
                pageResult.data.parsed_frames ?? pageResult.data.total_frames,
            }
          : null,
        summaryError: summaryResult.ok ? null : summaryResult.error,
        pageError: pageResult.ok ? null : pageResult.error,
      };

      setSpectrum((prev) =>
        buildCanTrafficSpectrum({
          ...slices,
          live: readCanLiveChip(useHostMetricsStore.getState().piMetrics),
          previous: prev,
          nowMs: Date.now(),
        }),
      );
      setLoading(false);
    };

    const tick = () => {
      void poll().finally(() => {
        if (!cancelled) {
          timer = setTimeout(tick, SPECTRUM_POLL_MS);
        }
      });
    };

    tick();

    return () => {
      cancelled = true;
      if (timer != null) {
        clearTimeout(timer);
      }
    };
  }, [active, sessionId]);

  useEffect(() => {
    if (!active) {
      return;
    }
    setSpectrum((prev) => ({
      ...prev,
      live: readCanLiveChip(piMetrics),
    }));
  }, [active, piMetrics]);

  if (!active) {
    return { ...spectrumRef.current, loading: loadingRef.current };
  }

  return { ...spectrum, loading };
}
