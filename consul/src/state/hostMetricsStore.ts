import { create } from 'zustand';

import type { HostMetrics } from '@/gen/marengo/v1/marengo_pb';

export type ChappeTransportMode = 'webtransport' | 'http-stream' | 'offline';

interface HostMetricsStore {
  transportMode: ChappeTransportMode;
  setTransportMode: (mode: ChappeTransportMode) => void;

  piMetrics: HostMetrics | null;
  piUpdatedAt: number | null;
  setPiMetrics: (metrics: HostMetrics | null) => void;

  jetsonMetrics: HostMetrics | null;
  jetsonUpdatedAt: number | null;
  setJetsonMetrics: (metrics: HostMetrics | null) => void;
}

export const useHostMetricsStore = create<HostMetricsStore>((set) => ({
  transportMode: 'offline',
  setTransportMode: (transportMode) => set({ transportMode }),

  piMetrics: null,
  piUpdatedAt: null,
  setPiMetrics: (piMetrics) =>
    set({ piMetrics, piUpdatedAt: piMetrics ? Date.now() : null }),

  jetsonMetrics: null,
  jetsonUpdatedAt: null,
  setJetsonMetrics: (jetsonMetrics) =>
    set({ jetsonMetrics, jetsonUpdatedAt: jetsonMetrics ? Date.now() : null }),
}));

export function hostMetricsStale(updatedAt: number | null, maxAgeMs = 5000): boolean {
  if (updatedAt === null) {
    return true;
  }
  return Date.now() - updatedAt > maxAgeMs;
}

export function canWarning(metrics: HostMetrics | null): boolean {
  if (!metrics?.network?.length) {
    return false;
  }
  return metrics.network.some(
    (iface) =>
      iface.name.startsWith('can') &&
      iface.canState.length > 0 &&
      iface.canState !== 'ERROR-ACTIVE',
  );
}

export function diskWarning(metrics: HostMetrics | null): boolean {
  return (
    metrics?.disks?.some((disk) => disk.readOnly || disk.nearlyFull) ?? false
  );
}

export function chappeDegraded(metrics: HostMetrics | null): boolean {
  if (!metrics?.chappe) {
    return false;
  }
  const ch = metrics.chappe;
  return !ch.ipcConnected || ch.lastPublishAgeMs > 3000;
}

export function clockUnsynced(metrics: HostMetrics | null): boolean {
  return metrics?.clock !== undefined && metrics.clock.synchronized === false;
}
