export function formatLogTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  const hours = date.getHours().toString().padStart(2, '0');
  const minutes = date.getMinutes().toString().padStart(2, '0');
  const seconds = date.getSeconds().toString().padStart(2, '0');
  const millis = date.getMilliseconds().toString().padStart(3, '0');

  return `${hours}:${minutes}:${seconds}.${millis}`;
}

const HIGHLIGHT_FIELD_KEYS = ['joint', 'error', 'operator', 'operator_id'] as const;

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
      out[key] = String(value);
    }
    return out;
  } catch {
    return {};
  }
}

export function highlightLogFieldKeys(fields: Record<string, string>): string[] {
  return HIGHLIGHT_FIELD_KEYS.filter((key) => key in fields);
}
