import * as React from 'react';
import { useRobotStore } from '@/state/robotStore';
import { useTestingStore } from '@/state/testingStore';
import { postHomeCommand } from '@/lib/gateway-api';
import { interpretPostEnableWatch } from '@/lib/enable-feedback';
import { useConfigSnapshot } from '@/hooks/use-config-snapshot';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { useActuatorZeroStore } from '@/state/actuatorZeroStore';
import { useActuatorStore } from '@/state/actuatorStore';
import { cn } from '@/lib/utils';

export function EnableDisableButtons() {
  const operationalMode = useRobotStore((s) => s.operationalMode);
  const robotState = useRobotStore((s) => s.robotState);
  const safetyState = useRobotStore((s) => s.safetyState);
  const { data: config = null } = useConfigSnapshot();
  const limitSnapshot = useActuatorStore((s) => s.limitSnapshot);
  const { enable, disable } = useTestingStore();
  const [homing, setHoming] = React.useState(false);
  const [enabling, setEnabling] = React.useState(false);
  const [homeError, setHomeError] = React.useState<string | null>(null);
  const [enableMessage, setEnableMessage] = React.useState<string | null>(null);
  const [enableKind, setEnableKind] = React.useState<'ok' | 'error' | 'pending'>(
    'pending',
  );
  const [enableWatchGen, setEnableWatchGen] = React.useState(0);
  const enableWatchStartedAt = React.useRef(0);

  const handleHome = async () => {
    setHoming(true);
    setHomeError(null);
    setEnableMessage(null);
    try {
      // HomingComplete → Davout READY when all joints are Verified (Set Zero).
      await postHomeCommand();
      // Sync Consul unlock with Pi Verified (zeros done outside this UI still count).
      const names = robotState?.joints.map((j) => j.name) ?? [];
      if (names.length > 0) {
        useActuatorZeroStore.getState().markAllZeroed(names);
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Home failed';
      setHomeError(message);
      console.error('Home command failed:', e);
    } finally {
      setHoming(false);
    }
  };

  const handleEnable = async () => {
    setEnabling(true);
    setHomeError(null);
    setEnableMessage('Enable queued — waiting for ACTIVE…');
    setEnableKind('pending');
    try {
      await enable();
      enableWatchStartedAt.current = Date.now();
      setEnableWatchGen((g) => g + 1);
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Enable failed';
      setEnableMessage(message);
      setEnableKind('error');
      console.error('Enable command failed:', e);
    } finally {
      setEnabling(false);
    }
  };

  const handleDisable = () => {
    // Cancel post-enable watch so intentional Disable does not look like a
    // failed Enable (watch would otherwise re-diagnose DISABLED → error).
    setEnableWatchGen(0);
    setEnableMessage(null);
    setEnableKind('pending');
    void disable();
  };

  React.useEffect(() => {
    if (enableWatchGen === 0) return;
    const startedAt = enableWatchStartedAt.current;
    const tick = () => {
      const elapsedMs = Date.now() - startedAt;
      const result = interpretPostEnableWatch({
        elapsedMs,
        operationalMode: useRobotStore.getState().operationalMode,
        safetyState: useRobotStore.getState().safetyState,
        robotState: useRobotStore.getState().robotState,
        config,
        limitSnapshot: useActuatorStore.getState().limitSnapshot,
      });
      if (result.message) {
        setEnableMessage(result.message);
        setEnableKind(result.kind);
      }
      return result.done;
    };

    if (tick()) return;

    const id = window.setInterval(() => {
      if (tick()) {
        window.clearInterval(id);
      }
    }, 150);
    return () => window.clearInterval(id);
  }, [enableWatchGen, operationalMode, safetyState, robotState, config, limitSnapshot]);

  const enableBlocked =
    operationalMode !== null && operationalMode !== 'READY';
  const enableHint =
    operationalMode === 'ACTIVE'
      ? null
      : operationalMode === 'DISABLED' || operationalMode === null
        ? 'Home first — marks READY after all joints are Verified (Set Zero in Inventory).'
        : null;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-4">
        <Badge variant={operationalMode === 'ACTIVE' ? 'default' : 'outline'}>
          {operationalMode || 'DISABLED'}
        </Badge>
        <Button
          variant="default"
          className="panel-brackets"
          onClick={() => void handleHome()}
          disabled={homing || operationalMode === 'ACTIVE'}
          title="Mark homing complete → READY (requires Verified zeros)"
        >
          {homing ? 'Homing…' : 'Home'}
        </Button>
        <Button
          className="panel-brackets"
          disabled={operationalMode !== 'READY' || enabling}
          onClick={() => void handleEnable()}
          title={
            enableBlocked
              ? 'Enable requires READY — use Home after Set Zero'
              : 'Enable motors'
          }
        >
          {enabling ? 'Enabling…' : 'Enable'}
        </Button>
        <Button variant="destructive" onClick={handleDisable}>
          Disable
        </Button>
      </div>
      {enableHint && !enableMessage ? (
        <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
          {enableHint}
        </p>
      ) : null}
      {homeError ? (
        <p className="text-xs text-destructive" role="alert">
          {homeError}
        </p>
      ) : null}
      {enableMessage ? (
        <p
          className={cn(
            'text-xs',
            enableKind === 'error' && 'text-destructive',
            enableKind === 'ok' && 'text-[color:var(--ok)]',
            enableKind === 'pending' && 'text-muted-foreground',
          )}
          role={enableKind === 'error' ? 'alert' : 'status'}
        >
          {enableMessage}
        </p>
      ) : null}
    </div>
  );
}
