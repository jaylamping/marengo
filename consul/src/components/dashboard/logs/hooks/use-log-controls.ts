import { useSyncExternalStore } from 'react';

import {
  getLogLive,
  getLogPaused,
  logBuffer,
  setLogLive,
  setLogPaused,
  subscribeLogLive,
  clearLogs,
  type LogBufferSnapshot,
} from '@/lib/log-buffer';

export function useLogLive() {
  return useSyncExternalStore(subscribeLogLive, getLogLive, getLogLive);
}

export function useLogPaused() {
  return useSyncExternalStore(
    subscribeLogLive,
    getLogPaused,
    getLogPaused,
  );
}

export function useLogBufferSnapshot(): LogBufferSnapshot {
  return useSyncExternalStore(
    logBuffer.subscribe,
    logBuffer.getSnapshot,
    logBuffer.getSnapshot,
  );
}

export function useLogActions() {
  return {
    setLive: setLogLive,
    setPaused: setLogPaused,
    clear: clearLogs,
  };
}
