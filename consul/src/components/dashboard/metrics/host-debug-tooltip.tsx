import { InformationCircleIcon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import type { HostDebugLine } from '@/lib/host-debug-info';

type HostDebugTooltipProps = {
  lines: HostDebugLine[];
};

export function HostDebugTooltip({ lines }: HostDebugTooltipProps) {
  if (lines.length === 0) {
    return null;
  }

  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            className="inline-flex size-5 shrink-0 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            aria-label="Host debug details"
          >
            <HugeiconsIcon
              icon={InformationCircleIcon}
              strokeWidth={2}
              className="size-3.5"
            />
          </button>
        }
      />
      <TooltipContent
        side="top"
        align="start"
        className="flex max-w-xs flex-col items-start gap-1 py-2"
      >
        {lines.map((line) => (
          <div
            key={line.label}
            className="grid w-full grid-cols-[auto_1fr] gap-x-2 gap-y-0.5 text-left"
          >
            <span className="text-muted-foreground">{line.label}</span>
            <span className="break-all font-mono">{line.value}</span>
          </div>
        ))}
      </TooltipContent>
    </Tooltip>
  );
}
