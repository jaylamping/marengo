import { useCallback } from 'react';
import { create } from '@bufbuild/protobuf';

import { TuningSlider } from '@/components/dashboard/actuators/tuning-slider';
import {
  ActuatorCommandSchema,
  OperatorCommandSchema,
  TuningChangeSchema,
  TuningTier,
} from '@/gen/marengo/v1/marengo_pb';
import { postActuatorCommand } from '@/lib/actuator-client';
import {
  kdMaxForJoint,
  kpMaxForJoint,
  resolveJointLimits,
  useActuatorStore,
} from '@/state/actuatorStore';

type TuningPanelProps = {
  jointName: string;
  wired: boolean;
};

export function TuningPanel({ jointName, wired }: TuningPanelProps) {
  const sessionId = useActuatorStore((s) => s.sessionId);
  const limitSnapshot = useActuatorStore((s) => s.limitSnapshot);
  const limits = resolveJointLimits(jointName, limitSnapshot);
  const kpMax = kpMaxForJoint(jointName, limitSnapshot);
  const kdMax = kdMaxForJoint(jointName, limitSnapshot);

  const sendTuning = useCallback(
    (param: 'kp' | 'kd', value: number) => {
      if (!wired || !sessionId || kpMax === null || kdMax === null) {
        return;
      }
      const capped =
        param === 'kp'
          ? Math.min(value, kpMax)
          : Math.min(value, kdMax);
      const command = create(OperatorCommandSchema, {
        timestampMs: BigInt(Date.now()),
        sessionId,
        operatorId: 'consul',
        seq: 1n,
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
      void postActuatorCommand(command).catch(() => {
        // Gateway may reject until PR-3 overlay lands; UI still debounces locally.
      });
    },
    [wired, sessionId, jointName, kpMax, kdMax],
  );

  if (!wired) {
    return null;
  }

  if (!limits || kpMax === null || kdMax === null) {
    return (
      <p className="text-sm text-muted-foreground" data-testid="tuning-panel-unavailable">
        Tuning limits unavailable
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
        Runtime MIT (diagnostic)
      </p>
      <TuningSlider
        label="Runtime kp"
        value={Math.min(10, kpMax)}
        max={kpMax}
        step={0.5}
        onDebouncedChange={(value) => sendTuning('kp', value)}
      />
      <TuningSlider
        label="Runtime kd"
        value={Math.min(1, kdMax)}
        max={kdMax}
        step={0.1}
        onDebouncedChange={(value) => sendTuning('kd', value)}
      />
      {!sessionId ? (
        <p className="text-xs text-muted-foreground">Session pending…</p>
      ) : null}
    </section>
  );
}
