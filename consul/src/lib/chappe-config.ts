/** Consul ↔ marengo-gateway endpoints (see docs/decisions/0008). */

export type ChappeEndpoints = {
  httpUrl: string;
  webTransportUrl: string;
};

export function getChappeEndpoints(): ChappeEndpoints | null {
  const httpUrl = import.meta.env.VITE_CHAPPE_HTTP_URL as string | undefined;
  const webTransportUrl = import.meta.env.VITE_CHAPPE_WEBTRANSPORT_URL as
    | string
    | undefined;
  if (!httpUrl?.trim() || !webTransportUrl?.trim()) {
    return null;
  }
  return {
    httpUrl: httpUrl.replace(/\/$/, ''),
    webTransportUrl: webTransportUrl.replace(/\/$/, ''),
  };
}

export function isChappeLive(): boolean {
  return getChappeEndpoints() !== null;
}

export const CHAPPE_TOPICS = {
  state: 'robot/state',
  safety: 'robot/safety',
  heartbeat: 'robot/heartbeat',
  logs: 'logs/structured',
  enable: 'robot/enable',
} as const;
