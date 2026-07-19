// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';

import { hydrateLogsFromSnapshot, logBuffer } from '@/lib/log-buffer';

describe('hydrateLogsFromSnapshot', () => {
  it('assigns unique React keys even when store ids repeat', () => {
    logBuffer.clear();
    hydrateLogsFromSnapshot([
      {
        id: 633,
        timestamp_ms: 1,
        level: 'info',
        target: 'a',
        message: 'first',
      },
      {
        id: 633,
        timestamp_ms: 2,
        level: 'info',
        target: 'a',
        message: 'second',
      },
    ]);

    // Chunked insert runs first chunk synchronously.
    const ids = [0, 1].map((i) => logBuffer.getEntry(i)?.id);
    expect(ids[0]).toBe('snap-633-0');
    expect(ids[1]).toBe('snap-633-1');
    expect(new Set(ids).size).toBe(2);
  });

  it('aborts a prior chunked hydrate when clear starts a new one', () => {
    logBuffer.clear();
    const first = Array.from({ length: 250 }, (_, i) => ({
      id: i + 1,
      timestamp_ms: i,
      level: 'info',
      target: 't',
      message: `m-${i}`,
    }));
    const second = Array.from({ length: 3 }, (_, i) => ({
      id: 9000 + i,
      timestamp_ms: i,
      level: 'warn',
      target: 't',
      message: `late-${i}`,
    }));

    hydrateLogsFromSnapshot(first);
    hydrateLogsFromSnapshot(second);

    expect(logBuffer.getCount()).toBe(3);
    expect(logBuffer.getEntry(0)?.id).toBe('snap-9000-0');
    expect(logBuffer.getEntry(2)?.message).toBe('late-2');
  });
});
