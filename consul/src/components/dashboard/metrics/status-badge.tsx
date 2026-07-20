import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

export type StatusTone = 'healthy' | 'warning' | 'fault' | 'muted';

const toneClasses: Record<StatusTone, string> = {
  healthy: 'border-ok/40 text-ok',
  warning: 'border-warning/40 text-warning',
  fault: 'border-fault/40 text-fault',
  muted: 'border-muted-foreground/40 text-muted-foreground',
};

type StatusBadgeProps = {
  label: string;
  tone: StatusTone;
  className?: string;
};

export function StatusBadge({ label, tone, className }: StatusBadgeProps) {
  return (
    <Badge variant="outline" className={cn(toneClasses[tone], className)}>
      {label}
    </Badge>
  );
}

export { toneClasses as statusToneClasses };
