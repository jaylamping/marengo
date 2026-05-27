import {
  generateLiveLogBatch,
  generateLogEntries,
  type LogEntry,
  type LogLevel,
} from '@/data/logs';

export const MAX_LOG_COUNT = 50_000;
export const INITIAL_LOG_SEED_COUNT = 10_000;

export type LevelCounts = Record<LogLevel, number>;

export type LogBufferSnapshot = {
  version: number;
  count: number;
  levelCounts: LevelCounts;
};

function emptyLevelCounts(): LevelCounts {
  return { DEBUG: 0, INFO: 0, WARN: 0, ERROR: 0, FATAL: 0 };
}

class LogBuffer {
  private slots: Array<LogEntry | undefined> = [];
  private head = 0;
  private size = 0;
  private version = 0;
  private levelCounts = emptyLevelCounts();
  private listeners = new Set<() => void>();
  private snapshot: LogBufferSnapshot = {
    version: 0,
    count: 0,
    levelCounts: this.levelCounts,
  };

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  private rebuildSnapshot() {
    this.snapshot = {
      version: this.version,
      count: this.size,
      levelCounts: this.levelCounts,
    };
  }

  private notify() {
    this.version += 1;
    this.rebuildSnapshot();
    for (const listener of this.listeners) {
      listener();
    }
  }

  private adjustLevel(level: LogLevel, delta: number) {
    this.levelCounts[level] += delta;
  }

  private slotIndex(logicalIndex: number): number {
    return (this.head + logicalIndex) % MAX_LOG_COUNT;
  }

  getSnapshot = (): LogBufferSnapshot => this.snapshot;

  getVersion(): number {
    return this.version;
  }

  getCount(): number {
    return this.size;
  }

  getEntry(logicalIndex: number): LogEntry | undefined {
    if (logicalIndex < 0 || logicalIndex >= this.size) {
      return undefined;
    }

    return this.slots[this.slotIndex(logicalIndex)];
  }

  getLevelCounts(): LevelCounts {
    return this.levelCounts;
  }

  seed(count: number) {
    const entries = generateLogEntries(count);
    this.slots = [];
    this.head = 0;
    this.size = 0;
    this.levelCounts = emptyLevelCounts();
    this.appendBatch(entries);
  }

  clear() {
    this.slots = [];
    this.head = 0;
    this.size = 0;
    this.levelCounts = emptyLevelCounts();
    this.notify();
  }

  appendBatch(batch: readonly LogEntry[]) {
    if (batch.length === 0) {
      return;
    }

    for (const entry of batch) {
      if (this.size < MAX_LOG_COUNT) {
        const tail = this.slotIndex(this.size);
        this.slots[tail] = entry;
        this.size += 1;
        this.adjustLevel(entry.level, 1);
        continue;
      }

      const removed = this.slots[this.head];
      if (removed) {
        this.adjustLevel(removed.level, -1);
      }

      this.slots[this.head] = entry;
      this.adjustLevel(entry.level, 1);
      this.head = (this.head + 1) % MAX_LOG_COUNT;
    }

    this.notify();
  }
}

export const logBuffer = new LogBuffer();

let streamSequence = 0;
let live = false;
let chappeLive = false;
let liveTimer: number | undefined;
const liveListeners = new Set<() => void>();

function notifyLiveListeners() {
  for (const listener of liveListeners) {
    listener();
  }
}

function stopLiveStream() {
  if (liveTimer !== undefined) {
    window.clearInterval(liveTimer);
    liveTimer = undefined;
  }
}

function startLiveStream() {
  if (liveTimer !== undefined || chappeLive) {
    return;
  }

  liveTimer = window.setInterval(() => {
    const batch = generateLiveLogBatch(4, streamSequence);
    streamSequence += batch.length;
    logBuffer.appendBatch(batch);
  }, 1000);
}

export function getLogLive(): boolean {
  return live;
}

export function setLogLive(next: boolean) {
  if (live === next) {
    return;
  }

  live = next;
  if (live) {
    startLiveStream();
  } else {
    stopLiveStream();
  }

  notifyLiveListeners();
}

export function subscribeLogLive(listener: () => void): () => void {
  liveListeners.add(listener);
  return () => liveListeners.delete(listener);
}

export function clearLogs() {
  streamSequence = 0;
  logBuffer.clear();
}

export function seedLogs(count: number) {
  streamSequence = count;
  logBuffer.seed(count);
}

export function appendLiveLog(entry: Omit<LogEntry, 'id'>) {
  streamSequence += 1;
  logBuffer.appendBatch([
    {
      id: `live-${streamSequence}`,
      ...entry,
    },
  ]);
}

export function enableChappeLiveLogs() {
  chappeLive = true;
  stopLiveStream();
  if (logBuffer.getCount() > 0 && logBuffer.getEntry(0)?.id.startsWith('seed-')) {
    logBuffer.clear();
    streamSequence = 0;
  }
}

export function ensureLogsSeeded(count = INITIAL_LOG_SEED_COUNT) {
  if (chappeLive) {
    return;
  }
  if (logBuffer.getCount() === 0) {
    seedLogs(count);
  }
}
