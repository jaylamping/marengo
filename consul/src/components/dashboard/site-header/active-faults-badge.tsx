import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { formatSafetyFaults } from '@/lib/enable-feedback';
import { useRobotStore } from '@/state/robotStore';

/**
 * `SafetyState.active_faults` on every route (UI-07): the count reads at a
 * glance, and each fault's joint, code, message and severity are in the
 * label and tooltip. Nothing renders while the list is empty.
 */
export function ActiveFaultsBadge() {
  const faults = useRobotStore((s) => s.safetyState?.activeFaults ?? []);
  if (faults.length === 0) {
    return null;
  }
  const detail = formatSafetyFaults(faults);
  const label = faults.length === 1 ? '1 FAULT' : `${faults.length} FAULTS`;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          tabIndex={0}
          data-testid="active-faults"
          aria-label={`Active faults: ${detail}`}
          className="inline-flex cursor-help items-center gap-2 border border-fault/50 bg-surface-0 px-2.5 py-1 outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <span className="led led-fault" aria-hidden />
          <span className="font-mono text-xs font-medium uppercase tracking-[0.14em] leading-none text-fault">
            {label}
          </span>
        </span>
      </TooltipTrigger>
      <TooltipContent side="bottom" align="end" className="max-w-xs py-2 leading-relaxed">
        <ul className="flex flex-col gap-1 text-left">
          {faults.map((fault, index) => (
            <li key={`${fault.joint}-${fault.code}-${index}`}>{formatSafetyFaults([fault])}</li>
          ))}
        </ul>
      </TooltipContent>
    </Tooltip>
  );
}
