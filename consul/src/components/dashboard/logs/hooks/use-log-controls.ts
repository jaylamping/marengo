import { useSyncExternalStore } from 'react';

import {
  getLogLive,
  logBuffer,
  setLogLive,
  subscribeLogLive,
  clearLogs,
  type LogBufferSnapshot,
} from '@/lib/log-buffer';

export function useLogLive() {
  return useSyncExternalStore(subscribeLogLive, getLogLive, getLogLive);
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
    clear: clearLogs,
  };
}
