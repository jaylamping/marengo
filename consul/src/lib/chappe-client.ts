export {
  type ChappeTelemetryHandlers,
  dispatchEnvelope,
  webTransportAvailable,
  connectWebTransport,
  connectHttpStream,
} from './chappe-transport';

export {
  fetchGatewayHealth,
  postEnableCommand,
  postMitCommandBatch,
  postTestingMitCommandBatch,
  postHomeCommand,
} from './gateway-api';

import {
  type ChappeTelemetryHandlers,
  webTransportAvailable,
  connectWebTransport,
  connectHttpStream,
} from './chappe-transport';
import { getChappeEndpoints, getChappeSubscribeTopics } from '@/lib/chappe-config';

/** Subscribe to live Chappe topics; WebTransport primary, HTTP stream fallback. */
export async function connectChappeStream(
  handlers: ChappeTelemetryHandlers,
): Promise<() => void> {
  const endpoints = getChappeEndpoints();
  if (!endpoints) {
    handlers.onTransportMode?.('offline');
    return () => {};
  }

  let closed = false;
  let cleanup: (() => void) | undefined;
  let reconnectTimer: number | undefined;
  let backoffMs = 1000;
  const maxBackoffMs = 30_000;

  const isClosed = () => closed;

  const wrappedHandlers: ChappeTelemetryHandlers = {
    ...handlers,
    onConnected: () => {
      backoffMs = 1000;
      handlers.onConnected?.();
    },
    onDisconnected: () => {
      handlers.onDisconnected?.();
      if (closed || reconnectTimer !== undefined) {
        return;
      }
      reconnectTimer = window.setTimeout(() => {
        reconnectTimer = undefined;
        void attemptConnect();
      }, backoffMs);
      backoffMs = Math.min(backoffMs * 2, maxBackoffMs);
    },
  };

  async function attemptConnect() {
    if (closed) {
      return;
    }
    cleanup?.();
    cleanup = undefined;
    try {
      if (webTransportAvailable()) {
        try {
          cleanup =
            (await connectWebTransport(wrappedHandlers, isClosed)) ?? undefined;
        } catch (err) {
          wrappedHandlers.onError?.(
            err instanceof Error ? err.message : String(err),
          );
        }
      }
      if (!cleanup) {
        cleanup =
          (await connectHttpStream(wrappedHandlers, isClosed)) ?? undefined;
      }
      if (!cleanup) {
        wrappedHandlers.onTransportMode?.('offline');
        wrappedHandlers.onError?.('no Chappe transport available');
        wrappedHandlers.onDisconnected?.();
      }
    } catch (err) {
      wrappedHandlers.onError?.(
        err instanceof Error ? err.message : String(err),
      );
      wrappedHandlers.onTransportMode?.('offline');
      wrappedHandlers.onDisconnected?.();
    }
  }

  await attemptConnect();

  return () => {
    closed = true;
    if (reconnectTimer !== undefined) {
      window.clearTimeout(reconnectTimer);
    }
    cleanup?.();
  };
}

/** @deprecated Use connectChappeStream */
export const connectChappeTelemetry = connectChappeStream;
