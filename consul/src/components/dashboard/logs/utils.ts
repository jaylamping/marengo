export function formatLogTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  const hours = date.getHours().toString().padStart(2, '0');
  const minutes = date.getMinutes().toString().padStart(2, '0');
  const seconds = date.getSeconds().toString().padStart(2, '0');
  const millis = date.getMilliseconds().toString().padStart(3, '0');

  return `${hours}:${minutes}:${seconds}.${millis}`;
}

const HIGHLIGHT_FIELD_KEYS = ['joint', 'error', 'operator', 'operator_id'] as const;

export type LogFieldEntry = {
  key: string;
  displayValue: string;
  highlighted: boolean;
};

export function splitTracingTarget(target: string): { crate?: string; module?: string } {
  const separator = target.indexOf('::');
  if (separator === -1) {
    return {};
  }

  return {
    crate: target.slice(0, separator),
    module: target.slice(separator + 2),
  };
}

export function formatLogTimestampLong(timestamp: number): string {
  return new Date(timestamp).toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    fractionalSecondDigits: 3,
  });
}

export function formatRelativeTimestamp(timestamp: number, now = Date.now()): string {
  const deltaMs = now - timestamp;
  if (deltaMs < 0) {
    return 'in the future';
  }
  if (deltaMs < 1_500) {
    return 'just now';
  }
  if (deltaMs < 60_000) {
    return `${Math.round(deltaMs / 1000)}s ago`;
  }
  if (deltaMs < 3_600_000) {
    return `${Math.round(deltaMs / 60_000)}m ago`;
  }
  if (deltaMs < 86_400_000) {
    return `${Math.round(deltaMs / 3_600_000)}h ago`;
  }

  return `${Math.round(deltaMs / 86_400_000)}d ago`;
}

function formatFieldValue(value: unknown): string {
  if (value === null || value === undefined) {
    return '';
  }
  if (typeof value === 'string') {
    return value;
  }
  if (typeof value === 'number' || typeof value === 'boolean') {
    return String(value);
  }

  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export function parseLogFieldEntries(fieldsJson?: string): LogFieldEntry[] {
  if (!fieldsJson?.trim()) {
    return [];
  }

  try {
    const parsed = JSON.parse(fieldsJson) as Record<string, unknown>;
    return Object.entries(parsed)
      .filter(([key]) => key !== '_truncated')
      .map(([key, value]) => ({
        key,
        displayValue: formatFieldValue(value),
        highlighted: HIGHLIGHT_FIELD_KEYS.includes(key as (typeof HIGHLIGHT_FIELD_KEYS)[number]),
      }))
      .sort((left, right) => {
        if (left.highlighted !== right.highlighted) {
          return left.highlighted ? -1 : 1;
        }

        return left.key.localeCompare(right.key);
      });
  } catch {
    return [];
  }
}

export function parseLogFields(fieldsJson?: string): Record<string, string> {
  if (!fieldsJson?.trim()) {
    return {};
  }
  try {
    const parsed = JSON.parse(fieldsJson) as Record<string, unknown>;
    const out: Record<string, string> = {};
    for (const [key, value] of Object.entries(parsed)) {
      if (value === null || value === undefined) {
        continue;
      }
      out[key] = formatFieldValue(value);
    }
    return out;
  } catch {
    return {};
  }
}

export function highlightLogFieldKeys(fields: Record<string, string>): string[] {
  return HIGHLIGHT_FIELD_KEYS.filter((key) => key in fields);
}
