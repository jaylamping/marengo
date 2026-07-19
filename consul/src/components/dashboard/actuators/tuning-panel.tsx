import { useCallback, useState } from 'react';
import { create } from '@bufbuild/protobuf';

import { TuningSlider } from '@/components/dashboard/actuators/tuning-slider';
import {
  ActuatorCommandSchema,
  OperatorCommandSchema,
  TuningChangeSchema,
  TuningTier,
} from '@/gen/marengo/v1/marengo_pb';
import { postActuatorCommand } from '@/lib/gateway-api';
import {
  jointLimitMax,
  liveJointLimits,
  selectClientId,
  useActuatorStore,
} from '@/state/actuatorStore';

type TuningPanelProps = {
  jointName: string;
  wired: boolean;
};

export function TuningPanel({ jointName, wired }: TuningPanelProps) {
  const bootstrap = useActuatorStore((s) => s.bootstrap);
  const limitSnapshot = useActuatorStore((s) => s.limitSnapshot);
  const lastError = useActuatorStore((s) => s.lastError);
  const setLastError = useActuatorStore((s) => s.setLastError);
  const nextCommandSeq = useActuatorStore((s) => s.nextCommandSeq);
  const clientId = selectClientId(bootstrap);
  const liveLimits = liveJointLimits(jointName, limitSnapshot);
  const kpMax = jointLimitMax(jointName, limitSnapshot, 'kp');
  const kdMax = jointLimitMax(jointName, limitSnapshot, 'kd');
  const [kpValue, setKpValue] = useState(0);
  const [kdValue, setKdValue] = useState(0);

  const sendTuning = useCallback(
    (param: 'kp' | 'kd', value: number) => {
      if (!wired || !clientId || kpMax === null || kdMax === null || !liveLimits) {
        return;
      }
      const capped = param === 'kp' ? Math.min(value, kpMax) : Math.min(value, kdMax);
      if (param === 'kp') {
        setKpValue(capped);
      } else {
        setKdValue(capped);
      }
      const command = create(OperatorCommandSchema, {
        timestampMs: BigInt(Date.now()),
        sessionId: clientId,
        operatorId: 'consul',
        seq: nextCommandSeq(),
        command: create(ActuatorCommandSchema, {
          joint: jointName,
          payload: {
            case: 'tuning',
            value: create(TuningChangeSchema, {
              tier: TuningTier.RUNTIME_MIT,
              param,
              value: capped,
              persist: false,
            }),
          },
        }),
      });
      void postActuatorCommand(command)
        .then(() => setLastError(null))
        .catch((error: unknown) => {
          setLastError(error instanceof Error ? error.message : 'tuning command failed');
        });
    },
    [wired, clientId, jointName, kpMax, kdMax, liveLimits, nextCommandSeq, setLastError],
  );

  if (!wired) {
    return null;
  }

  if (!liveLimits || kpMax === null || kdMax === null) {
    return (
      <p className="text-sm text-muted-foreground" data-testid="tuning-panel-unavailable">
        Waiting for live actuator limits before tuning…
      </p>
    );
  }

  return (
    <section
      className="mt-3 flex flex-col gap-3 border-t border-border/60 pt-3"
      data-testid="tuning-panel"
      aria-label={`Runtime tuning for ${jointName}`}
    >
      <p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
        Runtime MIT tuning (writes live; motion gated until PR-5)
      </p>
      <TuningSlider
        label="Runtime kp"
        value={kpValue}
        max={kpMax}
        step={0.5}
        onDebouncedChange={(value) => sendTuning('kp', value)}
      />
      <TuningSlider
        label="Runtime kd"
        value={kdValue}
        max={kdMax}
        step={0.1}
        onDebouncedChange={(value) => sendTuning('kd', value)}
      />
      {!clientId ? (
        <p className="text-xs text-muted-foreground">Client id pending…</p>
      ) : null}
      {lastError ? (
        <p className="text-xs text-destructive" data-testid="tuning-last-error" role="alert">
          {lastError}
        </p>
      ) : null}
    </section>
  );
}
