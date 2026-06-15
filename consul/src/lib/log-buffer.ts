import {
  generateLiveLogBatch,
  generateLogEntries,
  type LogEntry,
  type LogLevel,
} from '@/data/logs';
export const MAX_LOG_COUNT = 5_000;
export const INITIAL_LOG_SEED_COUNT = 500;
export const SNAPSHOT_HYDRATE_LIMIT = 300;
const UI_NOTIFY_MS = 400;
const MAX_INGEST_PER_SEC = 10;
/** Cap protobuf decodes when live — ingest filter drops more after decode. */
const MAX_DECODE_PER_SEC = 12;

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
  private notifyTimer: number | undefined;
  private lastNotifyAt = 0;
  private pendingLive: LogEntry[] = [];
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

  private flushNotify() {
    this.version += 1;
    this.rebuildSnapshot();
    for (const listener of this.listeners) {
      listener();
    }
  }

  private scheduleNotify() {
    if (this.notifyTimer !== undefined) {
      return;
    }
    const elapsed = Date.now() - this.lastNotifyAt;
    const delay = Math.max(0, UI_NOTIFY_MS - elapsed);
    this.notifyTimer = window.setTimeout(() => {
      this.notifyTimer = undefined;
      this.lastNotifyAt = Date.now();
      this.flushPendingLive();
      this.flushNotify();
    }, delay);
  }

  private flushPendingLive() {
    if (this.pendingLive.length === 0) {
      return;
    }
    const batch = this.pendingLive;
    this.pendingLive = [];
    this.insertBatch(batch);
  }

  private insertBatch(batch: readonly LogEntry[]) {
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
  }

  queueLiveEntry(entry: LogEntry) {
    if (this.pendingLive.length >= 256) {
      const dropIndex = this.pendingLive.findIndex((row) => row.level === 'DEBUG');
      if (dropIndex >= 0) {
        this.pendingLive.splice(dropIndex, 1);
      } else {
        this.pendingLive.shift();
      }
    }
    this.pendingLive.push(entry);
    this.scheduleNotify();
  }

  insertBatchChunked(batch: readonly LogEntry[], onComplete: () => void) {
    const chunkSize = 100;
    let offset = 0;

    const step = () => {
      const end = Math.min(offset + chunkSize, batch.length);
      this.insertBatch(batch.slice(offset, end));
      offset = end;
      if (offset < batch.length) {
        requestAnimationFrame(step);
        return;
      }
      onComplete();
    };

    step();
  }

  private notifyNow() {
    if (this.notifyTimer !== undefined) {
      window.clearTimeout(this.notifyTimer);
      this.notifyTimer = undefined;
    }
    this.flushPendingLive();
    this.lastNotifyAt = Date.now();
    this.flushNotify();
  }

  publish = () => {
    this.notifyNow();
  };

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
    this.pendingLive = [];
    this.slots = [];
    this.head = 0;
    this.size = 0;
    this.levelCounts = emptyLevelCounts();
    this.notifyNow();
  }

  appendBatch(batch: readonly LogEntry[]) {
    if (batch.length === 0) {
      return;
    }

    this.insertBatch(batch);
    this.scheduleNotify();
  }
}

export const logBuffer = new LogBuffer();

let streamSequence = 0;
let live = false;
let chappeLive = false;
let paused = true;
let logsPageActive = false;
let ingestWindowStart = 0;
let ingestWindowCount = 0;
let decodeWindowStart = 0;
let decodeWindowCount = 0;
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
    const batch = generateLiveLogBatch(2, streamSequence);
    streamSequence += batch.length;
    logBuffer.appendBatch(batch);
  }, 2000);
}

function allowIngest(level: LogLevel): boolean {
  const now = Date.now();
  if (now - ingestWindowStart >= 1000) {
    ingestWindowStart = now;
    ingestWindowCount = 0;
  }
  ingestWindowCount += 1;
  if (ingestWindowCount <= MAX_INGEST_PER_SEC) {
    return true;
  }
  return level === 'WARN' || level === 'ERROR' || level === 'FATAL';
}

export function getLogPaused(): boolean {
  return paused;
}

export function setLogPaused(next: boolean) {
  paused = next;
  notifyLiveListeners();
}

export function getLogLive(): boolean {
  return live && !paused;
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

export function setLogsPageActive(active: boolean) {
  logsPageActive = active;
  if (active && chappeLive) {
    paused = true;
    notifyLiveListeners();
  }
}

export function shouldIngestLogEvents(): boolean {
  return logsPageActive && !paused;
}

export function shouldDecodeLogEvents(): boolean {
  if (!shouldIngestLogEvents()) {
    return false;
  }
  const now = Date.now();
  if (now - decodeWindowStart >= 1000) {
    decodeWindowStart = now;
    decodeWindowCount = 0;
  }
  if (decodeWindowCount >= MAX_DECODE_PER_SEC) {
    return false;
  }
  decodeWindowCount += 1;
  return true;
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
  if (!shouldIngestLogEvents()) {
    return;
  }
  if (chappeLive && entry.level === 'DEBUG') {
    return;
  }
  if (!allowIngest(entry.level)) {
    return;
  }
  streamSequence += 1;
  logBuffer.queueLiveEntry({
    id: `live-${streamSequence}`,
    ...entry,
  });
}

export function enableChappeLiveLogs() {
  chappeLive = true;
  paused = true;
  stopLiveStream();
  if (logBuffer.getCount() > 0 && logBuffer.getEntry(0)?.id.startsWith('seed-')) {
    logBuffer.clear();
    streamSequence = 0;
  }
  notifyLiveListeners();
}

export function hydrateLogsFromSnapshot(
  entries: Array<{
    id?: number;
    timestamp_ms: number;
    level: string;
    target: string;
    message: string;
    session_id?: string;
    fields_json?: string;
  }>,
) {
  if (entries.length === 0) {
    return;
  }
  const batch: LogEntry[] = entries
    .map((e, i) => ({
      id: e.id !== undefined ? `snap-${e.id}` : `snap-${i}-${e.timestamp_ms}`,
      timestamp: e.timestamp_ms,
      level: mapProtoLevel(e.level),
      source: e.target,
      message: e.message,
      fieldsJson: e.fields_json || undefined,
      sessionId: e.session_id || undefined,
      storeId: e.id,
    }))
    .filter((entry) => !chappeLive || entry.level !== 'DEBUG')
    .slice(-SNAPSHOT_HYDRATE_LIMIT);
  logBuffer.clear();
  streamSequence = batch.length;
  logBuffer.insertBatchChunked(batch, () => {
    logBuffer.publish();
  });
}

function mapProtoLevel(level: string): LogLevel {
  switch (level.toLowerCase()) {
    case 'trace':
    case 'debug':
      return 'DEBUG';
    case 'warn':
      return 'WARN';
    case 'error':
      return 'ERROR';
    case 'fatal':
      return 'FATAL';
    default:
      return 'INFO';
  }
}

export function ensureLogsSeeded(count = INITIAL_LOG_SEED_COUNT) {
  if (chappeLive) {
    return;
  }
  if (import.meta.env.DEV && logBuffer.getCount() === 0) {
    seedLogs(count);
  }
}
