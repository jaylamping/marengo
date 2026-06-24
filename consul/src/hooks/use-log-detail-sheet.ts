import { useState } from 'react';

import type { LogEntry } from '@/data/logs';

export function useLogDetailSheet() {
  const [selectedLog, setSelectedLog] = useState<LogEntry | null>(null);
  const [detailOpen, setDetailOpen] = useState(false);

  function handleSelectLog(entry: LogEntry) {
    setSelectedLog(entry);
    setDetailOpen(true);
  }

  return {
    selectedLog,
    detailOpen,
    setDetailOpen,
    handleSelectLog,
  };
}
