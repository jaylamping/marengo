import { useEffect } from 'react';

import { HostNodeRole, OperationalMode } from '@/gen/marengo/v1/marengo_pb';
import { connectChappeStream } from '@/lib/chappe-client';
import { isChappeLive } from '@/lib/chappe-config';
import {
  appendLiveLog,
  enableChappeLiveLogs,
} from '@/lib/log-buffer';
import type { LogLevel } from '@/data/logs';
import { useHostMetricsStore } from '@/state/hostMetricsStore';
import { useRobotStore } from '@/state/robotStore';

const MAX_CHART_POINTS = 120;

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

  useEffect(() => {
    if (!isChappeLive()) {
      return;
    }

    enableChappeLiveLogs();
    let dispose: (() => void) | undefined;

    void connectChappeStream({
      onConnected: () => {
        setConnected(true);
        setGatewayError(null);
      },
      onDisconnected: () => setConnected(false),
      onTransportMode: (mode) => setTransportMode(mode),
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
      onImuSample: (sample) => setImuSample(sample),
      onLogEvent: (event) => {
        appendLiveLog({
          timestamp: Number(event.timestampMs),
          level: mapLogLevel(event.level),
          source: event.target,
          message: event.message,
        });
      },
      onHostMetrics: (metrics, topic) => {
        const role =
          metrics.nodeRole === HostNodeRole.JETSON
            ? 'jetson'
            : metrics.nodeRole === HostNodeRole.PI
              ? 'pi'
              : topic.includes('jetson')
                ? 'jetson'
                : 'pi';
        if (role === 'jetson') {
          setJetsonMetrics(metrics);
        } else {
          setPiMetrics(metrics);
        }
      },
    }).then((fn) => {
      dispose = fn;
    });

    return () => {
      dispose?.();
      setConnected(false);
      setTransportMode('offline');
    };
  }, [
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
