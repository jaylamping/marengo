import {
  LOG_CONTEXT_MAX_BYTES,
  LOG_CONTEXT_MAX_LINES,
  type AutoLearnLogContext,
} from '@marengo/compound-auto-learn';
import {
  fetchRecentLogs,
  fetchStructuredLogs,
  type StructuredLogEntryDto,
} from '@/lib/log-api';

export type LogAttachResult =
  | { ok: true; context: AutoLearnLogContext }
  | { ok: false; message: string };

function summarizeEntry(entry: StructuredLogEntryDto): string {
  const parts = [
    entry.timestamp_ms,
    entry.level,
    entry.target,
    entry.message,
  ];
  let fields = '';
  try {
    const parsed = JSON.parse(entry.fields_json) as Record<string, unknown>;
    const keep: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(parsed)) {
      if (
        /fault|reject|limit|joint|error|code|watchdog|davout/i.test(k) &&
        (typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean')
      ) {
        keep[k] = v;
      }
    }
    if (Object.keys(keep).length) {
      fields = ` ${JSON.stringify(keep)}`;
    }
  } catch {
    /* ignore */
  }
  return `${parts.join(' ')}${fields}`;
}

function truncateLines(lines: string[]): { text: string; truncated: boolean } {
  let truncated = false;
  let kept = lines.slice(-LOG_CONTEXT_MAX_LINES);
  if (lines.length > kept.length) truncated = true;
  let text = kept.join('\n');
  const enc = new TextEncoder();
  while (enc.encode(text).length > LOG_CONTEXT_MAX_BYTES && kept.length > 1) {
    kept = kept.slice(1);
    truncated = true;
    text = kept.join('\n');
  }
  if (enc.encode(text).length > LOG_CONTEXT_MAX_BYTES) {
    truncated = true;
    text = text.slice(0, LOG_CONTEXT_MAX_BYTES);
  }
  return { text, truncated };
}

/** Allowlisted structured summary only (no raw bench/trace/candump). */
export async function buildAutoLearnLogContext(
  sinceMs: number | null,
): Promise<LogAttachResult> {
  try {
    const structured = await fetchStructuredLogs({ limit: 80 });
    let entries: StructuredLogEntryDto[];
    if (structured.ok) {
      entries = structured.data.entries;
    } else {
      const recent = await fetchRecentLogs(80);
      if (!recent.ok) {
        return { ok: false, message: 'Session logs unavailable' };
      }
      entries = recent.data;
    }

    const filtered =
      sinceMs != null
        ? entries.filter((e) => e.timestamp_ms >= sinceMs)
        : entries;
    const lines = filtered.map(summarizeEntry);
    lines.push('note: raw bench/trace/candump not attached');
    const { text, truncated } = truncateLines(lines);
    if (!text.trim()) {
      return { ok: false, message: 'No structured log lines in window' };
    }
    return {
      ok: true,
      context: {
        attachedAtMs: Date.now(),
        truncated,
        sinceMs,
        summaryText: text,
      },
    };
  } catch {
    return { ok: false, message: 'Session logs unavailable' };
  }
}
