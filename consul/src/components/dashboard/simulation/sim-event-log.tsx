import type { SimEvent } from '@/data/simulation';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { cn } from '@/lib/utils';

const levelClasses = {
  info: 'text-muted-foreground',
  warn: 'text-amber-400',
  error: 'text-red-400',
} as const;

type SimEventLogProps = {
  events: SimEvent[];
};

export function SimEventLog({ events }: SimEventLogProps) {
  return (
    <Card>
      <CardHeader>
        <CardDescription>Events</CardDescription>
        <CardTitle className="text-lg font-semibold">Sim log</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="max-h-48 space-y-2 overflow-y-auto rounded-md border bg-muted/20 p-3 font-mono text-xs">
          {events.map((event) => (
            <div key={event.id} className="flex gap-3">
              <span className="shrink-0 text-muted-foreground">{event.timestamp}</span>
              <span className={cn('uppercase', levelClasses[event.level])}>
                {event.level}
              </span>
              <span className="min-w-0 flex-1">{event.message}</span>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}
