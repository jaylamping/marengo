import { useEffect, useState } from 'react';

import {
  ACTIVITY_TICK_MS,
  SPECTRUM_POLL_MS,
  TAIL_PAGE_LIMIT,
  appendLinkActivity,
  candumpTailOffset,
  foldCaptureState,
  readCanLiveChip,
  withLiveChip,
  type CanLinkActivitySample,
  type CaptureState,
} from '@/lib/can-traffic-spectrum';
import { fetchCandumpPage, fetchCandumpSummary } from '@/lib/log-api';
import { useHostMetricsStore } from '@/state/hostMetricsStore';

export type UseCanTrafficSpectrumArgs = {
  active: boolean;
  sessionId?: string | 'latest';
};

export type CanTrafficSpectrumView = {
  capture: CaptureState;
  loading: boolean;
  /** Observed host-metrics rx/tx samples only (never fabricated history). */
  linkActivity: CanLinkActivitySample[];
};

function initialCapture(): CaptureState {
  return { status: 'empty', live: readCanLiveChip(null) };
}

export function useCanTrafficSpectrum({
  active,
  sessionId = 'latest',
}: UseCanTrafficSpectrumArgs): CanTrafficSpectrumView {
  const piMetrics = useHostMetricsStore((s) => s.piMetrics);
  const [capture, setCapture] = useState<CaptureState>(initialCapture);
  const [loading, setLoading] = useState(true);
  const [linkActivity, setLinkActivity] = useState<CanLinkActivitySample[]>([]);

  useEffect(() => {
    if (!active) {
      return;
    }

    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    setLoading(true);

    const poll = async () => {
      const summaryResult = await fetchCandumpSummary(sessionId);
      if (cancelled) {
        return;
      }

      let pageResult: Awaited<ReturnType<typeof fetchCandumpPage>> | null = null;
      if (summaryResult.ok && summaryResult.data.parsed_frames > 0) {
        const offset = candumpTailOffset(
          summaryResult.data.parsed_frames,
          TAIL_PAGE_LIMIT,
        );
        pageResult = await fetchCandumpPage(sessionId, offset, TAIL_PAGE_LIMIT);
      }
      if (cancelled) {
        return;
      }

      setCapture((previous) =>
        foldCaptureState({
          summaryResult,
          pageResult:
            pageResult == null
              ? null
              : pageResult.ok
                ? {
                    ok: true,
                    data: {
                      frames: pageResult.data.frames,
                      total:
                        pageResult.data.parsed_frames ??
                        pageResult.data.total_frames,
                    },
                  }
                : pageResult,
          live: readCanLiveChip(useHostMetricsStore.getState().piMetrics),
          previous,
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
    setCapture((prev) => withLiveChip(prev, readCanLiveChip(piMetrics)));
  }, [active, piMetrics]);

  useEffect(() => {
    if (!active) {
      return;
    }
    const sample = () => {
      const live = readCanLiveChip(useHostMetricsStore.getState().piMetrics);
      setLinkActivity((prev) =>
        appendLinkActivity(prev, {
          atMs: Date.now(),
          rxBps: live.rxBytesPerSec ?? 0,
          txBps: live.txBytesPerSec ?? 0,
        }),
      );
    };
    sample();
    const id = setInterval(sample, ACTIVITY_TICK_MS);
    return () => clearInterval(id);
  }, [active]);

  return { capture, loading, linkActivity };
}
