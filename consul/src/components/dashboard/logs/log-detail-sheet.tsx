import type { LogEntry } from '@/data/logs';
import { LOG_LEVEL_STYLES, logsSheetContentClassName } from '@/components/dashboard/logs/constants';
import {
  formatLogTimestamp,
  formatLogTimestampLong,
  formatRelativeTimestamp,
  parseLogFieldEntries,
  splitTracingTarget,
} from '@/components/dashboard/logs/utils';
import { Badge } from '@/components/ui/badge';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { cn } from '@/lib/utils';

type LogDetailSheetProps = {
  entry: LogEntry | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

function formatLogTimestampIso(timestamp: number): string {
  return new Date(timestamp).toISOString();
}

function DetailRow({
  label,
  value,
  mono = false,
  className,
}: {
  label: string;
  value: string;
  mono?: boolean;
  className?: string;
}) {
  if (!value.trim()) {
    return null;
  }

  return (
    <div className="grid grid-cols-[96px_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className={cn(mono && 'font-mono break-all', className)}>{value}</dd>
    </div>
  );
}

export function LogDetailSheet({ entry, open, onOpenChange }: LogDetailSheetProps) {
  const fieldEntries = entry ? parseLogFieldEntries(entry.fieldsJson) : [];
  const targetParts = entry ? splitTracingTarget(entry.source) : {};
  const fieldsTruncated = entry?.fieldsJson?.includes('"_truncated"');

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="right"
        showOverlay={false}
        className={cn('w-full border-l shadow-2xl sm:max-w-lg', logsSheetContentClassName)}
      >
        {entry ? (
          <>
            <SheetHeader>
              <SheetTitle className="flex items-center gap-2 font-mono text-sm">
                <Badge
                  variant="outline"
                  className={cn(
                    'px-1.5 py-0 text-[10px] uppercase',
                    LOG_LEVEL_STYLES[entry.level],
                  )}
                >
                  {entry.level}
                </Badge>
                <span className="truncate">{entry.source}</span>
              </SheetTitle>
              <SheetDescription className="font-mono tabular-nums">
                {formatLogTimestamp(entry.timestamp)} · {formatRelativeTimestamp(entry.timestamp)}
              </SheetDescription>
            </SheetHeader>
            <div className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto px-6 pb-6">
              <section>
                <h3 className="mb-2 text-xs font-medium text-muted-foreground">Message</h3>
                <p
                  className={cn(
                    'rounded-md border bg-muted/30 p-3 font-mono text-xs leading-relaxed break-words whitespace-pre-wrap',
                    LOG_LEVEL_STYLES[entry.level],
                  )}
                >
                  {entry.message}
                </p>
              </section>

              <section>
                <h3 className="mb-2 text-xs font-medium text-muted-foreground">Metadata</h3>
                <dl className="space-y-2 rounded-md border bg-muted/15 p-3">
                  <DetailRow label="Time" value={formatLogTimestampLong(entry.timestamp)} mono />
                  <DetailRow label="ISO" value={formatLogTimestampIso(entry.timestamp)} mono />
                  <DetailRow label="Relative" value={formatRelativeTimestamp(entry.timestamp)} />
                  <DetailRow label="Level" value={entry.level} />
                  <DetailRow label="Target" value={entry.source} mono />
                  {targetParts.crate ? (
                    <DetailRow label="Crate" value={targetParts.crate} mono />
                  ) : null}
                  {targetParts.module ? (
                    <DetailRow label="Module" value={targetParts.module} mono />
                  ) : null}
                  {entry.sessionId ? (
                    <DetailRow label="Session" value={entry.sessionId} mono />
                  ) : null}
                  {entry.storeId !== undefined ? (
                    <DetailRow label="Store ID" value={String(entry.storeId)} mono />
                  ) : null}
                  <DetailRow label="Entry ID" value={entry.id} mono />
                  <DetailRow
                    label="Size"
                    value={`${entry.message.length} chars${fieldEntries.length > 0 ? ` · ${fieldEntries.length} fields` : ''}`}
                  />
                </dl>
              </section>

              {fieldEntries.length > 0 ? (
                <section>
                  <h3 className="mb-2 text-xs font-medium text-muted-foreground">
                    Structured fields
                  </h3>
                  <dl className="space-y-2 text-xs">
                    {fieldEntries.map(({ key, displayValue, highlighted }) => (
                      <div
                        key={key}
                        className={cn(
                          'rounded-md border px-3 py-2',
                          highlighted ? 'border-primary/30 bg-primary/5' : 'bg-muted/20',
                        )}
                      >
                        <dt className="font-medium text-muted-foreground">{key}</dt>
                        <dd
                          className={cn(
                            'mt-1 break-all font-mono whitespace-pre-wrap',
                            highlighted && 'text-foreground',
                          )}
                        >
                          {displayValue}
                        </dd>
                      </div>
                    ))}
                  </dl>
                  {fieldsTruncated ? (
                    <p className="mt-2 text-xs text-amber-500">
                      Field payload was truncated on ingest (max 2 KB).
                    </p>
                  ) : null}
                </section>
              ) : null}

              {entry.fieldsJson?.trim() ? (
                <section>
                  <h3 className="mb-2 text-xs font-medium text-muted-foreground">Raw JSON</h3>
                  <pre className="max-h-56 overflow-auto rounded-md border bg-muted/30 p-3 font-mono text-[11px] leading-relaxed break-all whitespace-pre-wrap">
                    {entry.fieldsJson}
                  </pre>
                </section>
              ) : null}
            </div>
          </>
        ) : null}
      </SheetContent>
    </Sheet>
  );
}
