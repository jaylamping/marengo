/** Consul ↔ marengo-gateway endpoints (see docs/decisions/0008). */

export type ChappeEndpoints = {
  httpUrl: string;
  webTransportUrl: string;
};

const WEBTRANSPORT_PORT = '8443';

/** Robot-hosted Consul (HTTPS on gateway); same host, WebTransport on UDP :8443. */
function endpointsFromRobotHostedPage(): ChappeEndpoints | null {
  if (typeof window === 'undefined') {
    return null;
  }
  if (window.location.protocol !== 'https:') {
    return null;
  }
  const { hostname, origin } = window.location;
  if (!hostname || !origin) {
    return null;
  }
  return {
    httpUrl: origin.replace(/\/$/, ''),
    webTransportUrl: `https://${hostname}:${WEBTRANSPORT_PORT}/chappe`,
  };
}

export function getChappeEndpoints(): ChappeEndpoints | null {
  const httpUrl = import.meta.env.VITE_CHAPPE_HTTP_URL as string | undefined;
  const webTransportUrl = import.meta.env.VITE_CHAPPE_WEBTRANSPORT_URL as
    | string
    | undefined;
  if (httpUrl?.trim() && webTransportUrl?.trim()) {
    return {
      httpUrl: httpUrl.replace(/\/$/, ''),
      webTransportUrl: webTransportUrl.replace(/\/$/, ''),
    };
  }
  return endpointsFromRobotHostedPage();
}

export function isChappeLive(): boolean {
  return getChappeEndpoints() !== null;
}

export const CHAPPE_TOPICS = {
  state: 'robot/state',
  safety: 'robot/safety',
  heartbeat: 'robot/heartbeat',
  imuTorso: 'sensors/imu/torso',
  logs: 'logs/structured',
  hostMetricsPi: 'host/metrics/pi',
  hostMetricsJetson: 'host/metrics/jetson',
  enable: 'robot/enable',
} as const;

export function getChappeSubscribeTopics(): string[] {
  return [
    CHAPPE_TOPICS.state,
    CHAPPE_TOPICS.safety,
    CHAPPE_TOPICS.heartbeat,
    CHAPPE_TOPICS.imuTorso,
    CHAPPE_TOPICS.logs,
    CHAPPE_TOPICS.hostMetricsPi,
    CHAPPE_TOPICS.hostMetricsJetson,
  ];
}
