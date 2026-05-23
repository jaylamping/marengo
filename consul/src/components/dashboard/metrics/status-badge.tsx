import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

export type StatusTone = 'healthy' | 'warning' | 'muted';

const toneClasses: Record<StatusTone, string> = {
  healthy: 'border-green-500/40 text-green-400',
  warning: 'border-amber-500/40 text-amber-400',
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
