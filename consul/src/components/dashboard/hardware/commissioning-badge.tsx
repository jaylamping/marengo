import { Badge } from '@/components/ui/badge';
import {
  commissioningBadgeLabel,
  type CommissioningBadge,
} from '@/lib/commissioning';
import { cn } from '@/lib/utils';

const toneClass: Record<CommissioningBadge, string> = {
  Fault: 'border-fault/50 text-fault',
  OutOfLimits: 'border-warning/50 text-warning',
  Offline: 'border-border text-muted-foreground',
  Active: 'border-accent/50 text-accent',
  Ready: 'border-[color:var(--ok)]/40 text-[color:var(--ok)]',
  Online: 'border-border text-foreground',
  Unknown: 'border-border text-muted-foreground',
};

type Props = {
  badge: CommissioningBadge;
  className?: string;
  testId?: string;
};

export function CommissioningBadgeChip({
  badge,
  className,
  testId = 'commissioning-badge',
}: Props) {
  return (
    <Badge
      variant="outline"
      data-testid={testId}
      data-badge={badge}
      title="Commissioning facet badge"
      className={cn(
        'px-1.5 font-mono text-[10px] uppercase tracking-[0.04em]',
        toneClass[badge],
        className,
      )}
    >
      {commissioningBadgeLabel(badge)}
    </Badge>
  );
}
