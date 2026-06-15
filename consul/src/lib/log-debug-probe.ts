/** Debug-mode counters for log pipeline CPU investigation. */

type ProbeCounters = {
  chappeLogEvents: number;
  chappeDispatches: number;
  appendAccepted: number;
  appendRejectedPaused: number;
  appendRejectedInactive: number;
  appendRejectedDebug: number;
  appendRejectedRate: number;
  logDecodeSkipped: number;
  bufferNotify: number;
  bufferPendingMax: number;
  virtualTableRenders: number;
  scrollEffectRuns: number;
  visibleIndexRebuilds: number;
};

const counters: ProbeCounters = {
  chappeLogEvents: 0,
  chappeDispatches: 0,
  appendAccepted: 0,
  appendRejectedPaused: 0,
  appendRejectedInactive: 0,
  appendRejectedDebug: 0,
  appendRejectedRate: 0,
  logDecodeSkipped: 0,
  bufferNotify: 0,
  bufferPendingMax: 0,
  virtualTableRenders: 0,
  scrollEffectRuns: 0,
  visibleIndexRebuilds: 0,
};

let flushTimer: number | undefined;

function sendProbe(message: string, hypothesisId: string, data: Record<string, unknown>) {
  const payload = {
    sessionId: '10bfe7',
    runId: 'post-fix',
    hypothesisId,
    location: 'log-debug-probe.ts',
    message,
    data,
    timestamp: Date.now(),
  };
  if (typeof window !== 'undefined') {
    (window as unknown as { __logDebugCounters?: ProbeCounters }).__logDebugCounters = {
      ...counters,
    };
  }
  // #region agent log
  fetch('http://127.0.0.1:7623/ingest/8e402d31-7578-4104-8f81-5d5eb2ca0378', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-Debug-Session-Id': '10bfe7',
    },
    body: JSON.stringify(payload),
  }).catch(() => {});
  // #endregion
}

export function ensureLogDebugProbeStarted() {
  if (flushTimer !== undefined || typeof window === 'undefined') {
    return;
  }
  flushTimer = window.setInterval(() => {
    const snapshot = { ...counters };
    sendProbe('log pipeline 2s window', 'SUMMARY', snapshot);
    counters.chappeLogEvents = 0;
    counters.chappeDispatches = 0;
    counters.appendAccepted = 0;
    counters.appendRejectedPaused = 0;
    counters.appendRejectedInactive = 0;
    counters.appendRejectedDebug = 0;
    counters.appendRejectedRate = 0;
    counters.logDecodeSkipped = 0;
    counters.bufferNotify = 0;
    counters.bufferPendingMax = 0;
    counters.virtualTableRenders = 0;
    counters.scrollEffectRuns = 0;
    counters.visibleIndexRebuilds = 0;
  }, 2000);
}

export function probeChappeDispatch(messageType: string) {
  counters.chappeDispatches += 1;
  if (messageType === 'marengo.v1.LogEvent') {
    counters.chappeLogEvents += 1;
  }
}

export function probeLogDecodeSkipped() {
  counters.logDecodeSkipped += 1;
}

export function probeAppendAccepted() {
  counters.appendAccepted += 1;
}

export function probeAppendRejected(reason: 'paused' | 'inactive' | 'debug' | 'rate') {
  if (reason === 'paused') counters.appendRejectedPaused += 1;
  if (reason === 'inactive') counters.appendRejectedInactive += 1;
  if (reason === 'debug') counters.appendRejectedDebug += 1;
  if (reason === 'rate') counters.appendRejectedRate += 1;
}

export function probeBufferNotify(pendingSize: number) {
  counters.bufferNotify += 1;
  counters.bufferPendingMax = Math.max(counters.bufferPendingMax, pendingSize);
}

export function probeVirtualTableRender() {
  counters.virtualTableRenders += 1;
}

export function probeScrollEffect() {
  counters.scrollEffectRuns += 1;
}

export function probeVisibleIndexRebuild() {
  counters.visibleIndexRebuilds += 1;
}
