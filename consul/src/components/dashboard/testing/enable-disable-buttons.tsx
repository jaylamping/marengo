import { useRobotStore } from '@/state/robotStore';
import { Badge } from '@/components/ui/badge';

/**
 * Testing chrome status strip.
 * Enable / Home / Disable commissioning controls live on Hardware;
 * Testing keeps motion, go-to-zero, and E-stop only.
 */
export function EnableDisableButtons() {
  const operationalMode = useRobotStore((s) => s.operationalMode);

  return (
    <div className="flex flex-col gap-2" data-testid="testing-mode-strip">
      <div className="flex flex-wrap items-center gap-4">
        <Badge variant={operationalMode === 'ACTIVE' ? 'default' : 'outline'}>
          {operationalMode || 'DISABLED'}
        </Badge>
        <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
          Enable and Set Zero on Hardware · motion and E-stop here
        </p>
      </div>
    </div>
  );
}
