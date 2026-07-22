import { useEffect, useState } from 'react';

import { Button } from '@/components/ui/button';
import {
  actuatorHomeGate,
  postActuatorHome,
} from '@/lib/actuator-home';
import { useActuatorZeroStore } from '@/state/actuatorZeroStore';
import { useRobotStore } from '@/state/robotStore';

const EMPTY_JOINT_NAMES: string[] = [];

type HomeActuatorButtonProps = {
  jointName: string;
  interactive: boolean;
};

/**
 * Move one zero'd actuator to 0 rad (POSITION). Locked until Set Zero (or
 * READY/ACTIVE sync marks live joints zeroed) and motors are ACTIVE.
 */
export function HomeActuatorButton({
  jointName,
  interactive,
}: HomeActuatorButtonProps) {
  const connected = useRobotStore((s) => s.connected);
  const operationalMode = useRobotStore((s) => s.operationalMode);
  const robotState = useRobotStore((s) => s.robotState);
  const live = Boolean(robotState?.joints.some((j) => j.name === jointName));
  const zeroed = useActuatorZeroStore((s) => Boolean(s.zeroed[jointName]));
  const markZeroed = useActuatorZeroStore((s) => s.markZeroed);
  const markAllZeroed = useActuatorZeroStore((s) => s.markAllZeroed);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [okHint, setOkHint] = useState(false);

  // Davout READY/ACTIVE implies Verified joints — unlock Home for live set.
  useEffect(() => {
    if (operationalMode !== 'READY' && operationalMode !== 'ACTIVE') {
      return;
    }
    const names =
      robotState?.joints.map((j) => j.name) ?? EMPTY_JOINT_NAMES;
    if (names.length === 0) return;
    markAllZeroed(names);
  }, [operationalMode, robotState, markAllZeroed]);

  const gate = actuatorHomeGate({
    interactive,
    connected,
    live,
    jointName,
    operationalMode,
    zeroed,
  });

  useEffect(() => {
    if (!okHint) return;
    const t = window.setTimeout(() => setOkHint(false), 3000);
    return () => window.clearTimeout(t);
  }, [okHint]);

  const onHome = async () => {
    if (!gate.ok || busy) return;
    setBusy(true);
    setError(null);
    setOkHint(false);
    try {
      await postActuatorHome(jointName);
      setOkHint(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Home failed');
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <Button
        type="button"
        size="sm"
        variant="default"
        className="panel-brackets"
        disabled={!gate.ok || busy}
        onClick={() => void onHome()}
        title={gate.reason ?? 'Command this joint to 0 rad'}
      >
        {busy ? 'Homing…' : 'Home'}
      </Button>
      {!gate.ok && gate.reason ? (
        <p
          className="basis-full text-xs text-muted-foreground"
          role="status"
        >
          {gate.reason}
        </p>
      ) : null}
      {!zeroed &&
      interactive &&
      live &&
      operationalMode !== 'READY' &&
      operationalMode !== 'ACTIVE' ? (
        <Button
          type="button"
          size="sm"
          variant="outline"
          className="basis-full sm:basis-auto"
          onClick={() => markZeroed(jointName)}
          title="Use when this joint is already Verified on the Pi (Set Zero outside Consul)"
        >
          Unlock — already Verified
        </Button>
      ) : null}
      {error ? (
        <p className="basis-full text-xs text-destructive" role="alert">
          {error}
        </p>
      ) : null}
      {okHint ? (
        <p
          className="basis-full font-mono text-xs text-[color:var(--ok)]"
          role="status"
        >
          Home queued — target 0 rad
        </p>
      ) : null}
    </>
  );
}
