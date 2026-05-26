import { useEffect } from 'react';

import { OperationalMode } from '@/gen/marengo/v1/marengo_pb';
import { connectChappeTelemetry } from '@/lib/chappe-client';
import { isChappeLive } from '@/lib/chappe-config';
import { useRobotStore } from '@/state/robotStore';

const MAX_CHART_POINTS = 120;

export function useChappeTelemetry(): void {
  const setConnected = useRobotStore((s) => s.setConnected);
  const setOperationalMode = useRobotStore((s) => s.setOperationalMode);
  const setRobotState = useRobotStore((s) => s.setRobotState);
  const setSafetyState = useRobotStore((s) => s.setSafetyState);
  const appendTrackingPoint = useRobotStore((s) => s.appendTrackingPoint);
  const setGatewayError = useRobotStore((s) => s.setGatewayError);

  useEffect(() => {
    if (!isChappeLive()) {
      return;
    }

    let dispose: (() => void) | undefined;

    void connectChappeTelemetry({
      onConnected: () => {
        setConnected(true);
        setGatewayError(null);
      },
      onDisconnected: () => setConnected(false),
      onError: (message) => {
        setGatewayError(message);
        setConnected(false);
      },
      onRobotState: (state) => {
        setRobotState(state);
        const joint = state.joints[0];
        if (joint) {
          appendTrackingPoint({
            time: String(Number(state.timestampMs % 10_000n)),
            measured: joint.position,
            commanded: joint.position,
          });
        }
      },
      onSafetyState: (safety) => {
        setSafetyState(safety);
        const mode = safety.mode;
        if (mode === OperationalMode.ACTIVE) {
          setOperationalMode('ACTIVE');
        } else if (mode === OperationalMode.READY) {
          setOperationalMode('READY');
        } else if (mode === OperationalMode.DISABLED) {
          setOperationalMode('DISABLED');
        } else {
          setOperationalMode(null);
        }
      },
      onHeartbeat: () => {},
    }).then((fn) => {
      dispose = fn;
    });

    return () => {
      dispose?.();
      setConnected(false);
    };
  }, [
    appendTrackingPoint,
    setConnected,
    setGatewayError,
    setOperationalMode,
    setRobotState,
    setSafetyState,
  ]);
}

export function useTrackingSeries() {
  return useRobotStore((s) => s.trackingPoints);
}
