/** Consul ↔ marengo-gateway endpoints (see docs/decisions/0008). */

export type ChappeEndpoints = {
  httpUrl: string;
  webTransportUrl: string;
};

export type ChappeEndpointSource = 'baked' | 'derived' | 'none';

export type ChappeResolution = {
  endpoints: ChappeEndpoints | null;
  source: ChappeEndpointSource;
};

const WEBTRANSPORT_PORT = '8443';

const LOCALHOST_HOSTS = new Set(['127.0.0.1', 'localhost']);

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

function bakedEndpointsFromEnv(): ChappeEndpoints | null {
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
  return null;
}

export function resolveChappeEndpoints(): ChappeResolution {
  const baked = bakedEndpointsFromEnv();
  if (baked) {
    return { endpoints: baked, source: 'baked' };
  }
  const derived = endpointsFromRobotHostedPage();
  if (derived) {
    return { endpoints: derived, source: 'derived' };
  }
  return { endpoints: null, source: 'none' };
}

export function getChappeEndpoints(): ChappeEndpoints | null {
  return resolveChappeEndpoints().endpoints;
}

export function isChappeLive(): boolean {
  return resolveChappeEndpoints().endpoints !== null;
}

export function chappeMisconfigHint(): string | null {
  const { endpoints, source } = resolveChappeEndpoints();
  if (source !== 'baked' || !endpoints || typeof window === 'undefined') {
    return null;
  }

  let bakedHost: string;
  try {
    bakedHost = new URL(endpoints.httpUrl).hostname;
  } catch {
    return null;
  }

  const originHost = window.location.hostname;
  if (!originHost || bakedHost === originHost) {
    return null;
  }

  // Vite often serves on localhost while .env bakes 127.0.0.1 (or vice versa).
  if (LOCALHOST_HOSTS.has(bakedHost) && LOCALHOST_HOSTS.has(originHost)) {
    return null;
  }

  if (LOCALHOST_HOSTS.has(bakedHost)) {
    return `Baked dev URLs (${bakedHost}) — redeploy Consul with production env scrub`;
  }

  return `Baked Chappe host (${bakedHost}) does not match page origin (${originHost})`;
}

export function chappeConnectionErrDetail(
  resolution: ChappeResolution,
  misconfigHint: string | null,
): string | null {
  if (!resolution.endpoints) {
    return null;
  }

  const lines = [
    `HTTP: ${resolution.endpoints.httpUrl}`,
    `WebTransport: ${resolution.endpoints.webTransportUrl}`,
  ];

  if (misconfigHint) {
    lines.push(misconfigHint);
  } else if (resolution.source === 'derived') {
    lines.push('Gateway unreachable — check marengo-gateway on Pi');
  }

  return lines.join('\n');
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
