/** Log wireframe data — replace with Chappe / tracing stream later. */

export const LOG_LEVELS = ['DEBUG', 'INFO', 'WARN', 'ERROR', 'FATAL'] as const;

export type LogLevel = (typeof LOG_LEVELS)[number];

export type LogLevelFilter = 'all' | LogLevel;

export type LogEntry = {
  id: string;
  timestamp: number;
  level: LogLevel;
  source: string;
  message: string;
  fieldsJson?: string;
};

export const LOG_SOURCES = [
  'berthier',
  'davout',
  'chappe',
  'robstride',
  'consul',
  'fouche',
] as const;

const SAMPLE_MESSAGES: Record<LogLevel, string[]> = {
  DEBUG: [
    'CAN frame 0x7016 loc_ref ack in 0.4 ms',
    'Joint state cache refreshed for shoulder_pitch',
    'Heartbeat received from node 0x02',
  ],
  INFO: [
    'Control loop tick 500 Hz stable',
    'Preset golden_pose loaded',
    'Chappe session RTT 1.3 ms',
  ],
  WARN: [
    'Joint elbow_roll approaching soft limit',
    'CPU throttle flag set on Pi host',
    'Missed telemetry frame, recovered on retry',
  ],
  ERROR: [
    'CAN timeout on node 0x04 after 3 retries',
    'Safety gate blocked enable request',
    'Planner IK solve failed for target pose',
  ],
  FATAL: [
    'E-stop asserted — all enables dropped',
    'Bus voltage below minimum operating threshold',
  ],
};

function pickLevel(index: number): LogLevel {
  if (index % 997 === 0) {
    return 'FATAL';
  }
  if (index % 113 === 0) {
    return 'ERROR';
  }
  if (index % 37 === 0) {
    return 'WARN';
  }
  if (index % 4 === 0) {
    return 'DEBUG';
  }

  return 'INFO';
}

export function generateLogEntries(
  count: number,
  options?: { startTimestamp?: number; idOffset?: number },
): LogEntry[] {
  const startTimestamp = options?.startTimestamp ?? Date.now() - count * 50;
  const idOffset = options?.idOffset ?? 0;
  const entries: LogEntry[] = new Array(count);

  for (let index = 0; index < count; index += 1) {
    const level = pickLevel(index + idOffset);
    const messages = SAMPLE_MESSAGES[level];
    const message = messages[(index + idOffset) % messages.length] ?? messages[0] ?? '';
    const source = LOG_SOURCES[(index + idOffset) % LOG_SOURCES.length] ?? LOG_SOURCES[0];

    entries[index] = {
      id: `log-${idOffset + index}`,
      timestamp: startTimestamp + index * 50,
      level,
      source,
      message,
    };
  }

  return entries;
}

export function generateLiveLogBatch(batchSize: number, sequence: number): LogEntry[] {
  const now = Date.now();
  return generateLogEntries(batchSize, {
    startTimestamp: now - batchSize * 10,
    idOffset: sequence,
  });
}

export function countLogsByLevel(entries: readonly LogEntry[]): Record<LogLevel, number> {
  const counts: Record<LogLevel, number> = {
    DEBUG: 0,
    INFO: 0,
    WARN: 0,
    ERROR: 0,
    FATAL: 0,
  };

  for (const entry of entries) {
    counts[entry.level] += 1;
  }

  return counts;
}
