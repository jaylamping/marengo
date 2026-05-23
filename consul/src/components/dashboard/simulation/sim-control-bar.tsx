import { Button } from '@/components/ui/button';
import { HugeiconsIcon } from '@hugeicons/react';
import {
  PauseIcon,
  PlayIcon,
  RefreshIcon,
  StopIcon,
  StepOverIcon,
} from '@hugeicons/core-free-icons';

import type { SimSessionState } from '@/data/simulation';

type SimControlBarProps = {
  sessionState: SimSessionState;
};

export function SimControlBar({ sessionState }: SimControlBarProps) {
  const disconnected = sessionState === 'disconnected';

  return (
    <div className="flex flex-wrap items-center gap-2 rounded-lg border bg-card px-4 py-3">
      <Button size="sm" disabled={disconnected}>
        <HugeiconsIcon icon={PlayIcon} strokeWidth={2} data-icon="inline-start" />
        Play
      </Button>
      <Button size="sm" variant="secondary" disabled={disconnected}>
        <HugeiconsIcon icon={PauseIcon} strokeWidth={2} data-icon="inline-start" />
        Pause
      </Button>
      <Button size="sm" variant="secondary" disabled={disconnected}>
        <HugeiconsIcon icon={StopIcon} strokeWidth={2} data-icon="inline-start" />
        Stop
      </Button>
      <Button size="sm" variant="outline" disabled={disconnected}>
        <HugeiconsIcon icon={StepOverIcon} strokeWidth={2} data-icon="inline-start" />
        Step
      </Button>
      <Button size="sm" variant="outline" disabled={disconnected}>
        <HugeiconsIcon icon={RefreshIcon} strokeWidth={2} data-icon="inline-start" />
        Reset scene
      </Button>
      <span className="ml-auto text-xs text-muted-foreground">
        Controls wireframe — Isaac Lab RPC TBD
      </span>
    </div>
  );
}
