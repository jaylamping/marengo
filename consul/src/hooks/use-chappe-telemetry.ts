import { useEffect } from 'react';

import { HostNodeRole, OperationalMode, type HostMetrics, type RobotState } from '@/gen/marengo/v1/marengo_pb';
import { connectChappeStream } from '@/lib/chappe-client';
import { isChappeLive } from '@/lib/chappe-config';
import { appendLiveLog, enableChappeLiveLogs } from '@/lib/log-buffer';
import { publishTeachSampleFromRobotState } from '@/lib/teach-sample-bus';
import { throttleTrailing } from '@/lib/throttle-callback';
import { useGatewayCredential } from '@/hooks/use-runtime-credential';
import type { LogLevel } from '@/data/logs';
import { useHostMetricsStore } from '@/state/hostMetricsStore';
import { useRobotStore } from '@/state/robotStore';

const TELEMETRY_UI_MS = 100;

function applyRobotState(
  state: RobotState,
  setRobotState: (state: RobotState) => void,
  appendTrackingPoint: (
    jointName: string,
    point: {
      time: string;
      measured: number;
      velocity: number;
      torque: number;
      temperature: number;
    },
  ) => void,
) {
  setRobotState(state);
  for (const joint of state.joints) {
    appendTrackingPoint(joint.name, {
      time: String(Number(state.timestampMs % 10_000n)),
      measured: joint.position,
      velocity: joint.velocity,
      torque: joint.effort,
      temperature: joint.temperatureC,
    });
  }
}

function mapLogLevel(level: string): LogLevel {
  switch (level.toLowerCase()) {
    case 'trace':
    case 'debug':
      return 'DEBUG';
    case 'warn':
      return 'WARN';
    case 'error':
      return 'ERROR';
    case 'fatal':
      return 'FATAL';
    default:
      return 'INFO';
  }
}

export function useChappeTelemetry(): void {
  const setConnected = useRobotStore((s) => s.setConnected);
  const setOperationalMode = useRobotStore((s) => s.setOperationalMode);
  const setRobotState = useRobotStore((s) => s.setRobotState);
  const setSafetyState = useRobotStore((s) => s.setSafetyState);
  const setImuSample = useRobotStore((s) => s.setImuSample);
  const appendTrackingPoint = useRobotStore((s) => s.appendTrackingPoint);
  const setGatewayError = useRobotStore((s) => s.setGatewayError);
  const setTransportMode = useHostMetricsStore((s) => s.setTransportMode);
  const setPiMetrics = useHostMetricsStore((s) => s.setPiMetrics);
  const setJetsonMetrics = useHostMetricsStore((s) => s.setJetsonMetrics);

  const readCredential = useGatewayCredential('sensitiveRead');
  useEffect(() => {
    if (!isChappeLive()) {
      return;
    }

    enableChappeLiveLogs();
    let disposed = false;
    let dispose: (() => void) | undefined;

    const publishRobotState = throttleTrailing((state: RobotState) => {
      applyRobotState(state, setRobotState, appendTrackingPoint);
    }, TELEMETRY_UI_MS);

    const publishPiMetrics = throttleTrailing((metrics: HostMetrics) => {
      setPiMetrics(metrics);
    }, TELEMETRY_UI_MS);

    const publishJetsonMetrics = throttleTrailing((metrics: HostMetrics) => {
      setJetsonMetrics(metrics);
    }, TELEMETRY_UI_MS);

    const retireRuntimeFacts = () => {
      publishRobotState.cancel();
      publishPiMetrics.cancel();
      publishJetsonMetrics.cancel();
      setRobotState(null);
      setSafetyState(null);
      setImuSample(null);
      setOperationalMode(null);
      setPiMetrics(null);
      setJetsonMetrics(null);
      setConnected(false);
    };

    void connectChappeStream({
      onConnected: () => {
        if (disposed) return;
        retireRuntimeFacts();
        setGatewayError(null);
      },
      onDisconnected: () => { if (!disposed) retireRuntimeFacts(); },
      onRuntimeConnectionState: () => { if (!disposed) retireRuntimeFacts(); },
      onRuntimeObservationGap: () => { if (!disposed) retireRuntimeFacts(); },
      onTransportMode: (mode) => { if (!disposed) setTransportMode(mode); },
      onError: (message) => {
        if (disposed) return;
        setGatewayError(message);
        setConnected(false);
      },
      onRobotState: (state) => {
        if (disposed) return;
        setConnected(true);
        // Teach-record listens before UI throttle (~10 Hz store).
        publishTeachSampleFromRobotState(state);
        publishRobotState(state);
      },
      onSafetyState: (safety) => {
        if (disposed) return;
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
      onImuSample: (sample) => { if (!disposed) setImuSample(sample); },
      onLogEvent: (event) => {
        if (disposed) return;
        appendLiveLog({
          timestamp: Number(event.timestampMs),
          level: mapLogLevel(event.level),
          source: event.target,
          message: event.message,
          fieldsJson: event.fieldsJson || undefined,
          sessionId: event.sessionId || undefined,
        });
      },
      onHostMetrics: (metrics, topic) => {
        if (disposed) return;
        const role =
          metrics.nodeRole === HostNodeRole.JETSON
            ? 'jetson'
            : metrics.nodeRole === HostNodeRole.PI
              ? 'pi'
              : topic.includes('jetson')
                ? 'jetson'
                : 'pi';
        if (role === 'jetson') {
          publishJetsonMetrics(metrics);
        } else {
          publishPiMetrics(metrics);
        }
      },
    }).then((fn) => {
      if (!disposed) {
        dispose = fn;
      } else {
        fn?.();
      }
    });

    return () => {
      disposed = true;
      retireRuntimeFacts();
      dispose?.();
      setConnected(false);
      setTransportMode('offline');
    };
  }, [
    readCredential,
    appendTrackingPoint,
    setConnected,
    setGatewayError,
    setJetsonMetrics,
    setOperationalMode,
    setPiMetrics,
    setRobotState,
    setSafetyState,
    setImuSample,
    setTransportMode,
  ]);
}

export function useTrackingSeries() {
  return useRobotStore((s) => s.trackingPoints);
}
