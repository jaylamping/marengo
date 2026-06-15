import type { LogEntry } from '@/data/logs';
import { LOG_LEVEL_STYLES } from '@/components/dashboard/logs/constants';
import {
  formatLogTimestamp,
  parseLogFields,
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

export function LogDetailSheet({ entry, open, onOpenChange }: LogDetailSheetProps) {
  const fields = entry ? parseLogFields(entry.fieldsJson) : {};
  const fieldKeys = Object.keys(fields).filter((key) => key !== '_truncated');

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="w-full sm:max-w-md">
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
                {formatLogTimestamp(entry.timestamp)} · {formatLogTimestampIso(entry.timestamp)}
              </SheetDescription>
            </SheetHeader>
            <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-6 pb-6">
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
              {fieldKeys.length > 0 ? (
                <section>
                  <h3 className="mb-2 text-xs font-medium text-muted-foreground">Fields</h3>
                  <dl className="space-y-2 text-xs">
                    {fieldKeys.map((key) => (
                      <div key={key} className="rounded-md border bg-muted/20 px-3 py-2">
                        <dt className="font-medium text-muted-foreground">{key}</dt>
                        <dd className="mt-1 break-all font-mono">{fields[key]}</dd>
                      </div>
                    ))}
                  </dl>
                  {fields._truncated === 'true' ? (
                    <p className="mt-2 text-xs text-amber-500">Field payload was truncated on ingest.</p>
                  ) : null}
                </section>
              ) : null}
              {entry.fieldsJson?.trim() ? (
                <section>
                  <h3 className="mb-2 text-xs font-medium text-muted-foreground">Raw JSON</h3>
                  <pre className="max-h-48 overflow-auto rounded-md border bg-muted/30 p-3 font-mono text-[11px] leading-relaxed break-all whitespace-pre-wrap">
                    {entry.fieldsJson}
                  </pre>
                </section>
              ) : null}
              <section>
                <h3 className="mb-2 text-xs font-medium text-muted-foreground">Entry ID</h3>
                <p className="font-mono text-xs text-muted-foreground break-all">{entry.id}</p>
              </section>
            </div>
          </>
        ) : null}
      </SheetContent>
    </Sheet>
  );
}
