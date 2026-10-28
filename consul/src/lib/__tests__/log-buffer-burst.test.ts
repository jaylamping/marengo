// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';

import {
  appendLiveLog,
  enableChappeLiveLogs,
  logBuffer,
  setLogPaused,
  setLogsPageActive,
  shouldDecodeLogEvents,
} from '@/lib/log-buffer';

afterEach(() => {
  setLogsPageActive(false);
  logBuffer.clear();
});

describe('live log burst (UI-18)', () => {
  it('decodes every event of a burst, so no ERROR is dropped before its level is known', () => {
    enableChappeLiveLogs();
    setLogsPageActive(true);
    setLogPaused(false);
    logBuffer.clear();

    let decoded = 0;
    for (let i = 0; i < 30; i += 1) {
      // The transport decodes only when this says so; the level is unknown here.
      if (!shouldDecodeLogEvents()) continue;
      decoded += 1;
      appendLiveLog({ timestamp: i, level: 'ERROR', source: 'davout', message: `fault ${i}` });
    }
    expect(decoded).toBe(30);
  });
});
