import { create, fromBinary, toBinary } from '@bufbuild/protobuf';

import {
  EnableRequestSchema,
  EnvelopeSchema,
  GatewaySubscribeSchema,
  HeartbeatSchema,
  type Heartbeat,
  type HostMetrics,
  HostMetricsSchema,
  type ImuSample,
  ImuSampleSchema,
  type LogEvent,
  LogEventSchema,
  type RobotState,
  type SafetyState,
  RobotStateSchema,
  SafetyStateSchema,
} from '@/gen/marengo/v1/marengo_pb';
import {
  CHAPPE_TOPICS,
  getChappeEndpoints,
  getChappeSubscribeTopics,
} from '@/lib/chappe-config';
import type { ChappeTransportMode } from '@/state/hostMetricsStore';

type WebTransportCertificateHash = {
  algorithm: 'sha-256';
  value: Uint8Array;
};

export type ChappeTelemetryHandlers = {
  onRobotState: (state: RobotState) => void;
  onSafetyState: (state: SafetyState) => void;
  onHeartbeat: (heartbeat: Heartbeat) => void;
  onImuSample?: (sample: ImuSample) => void;
  onLogEvent?: (event: LogEvent) => void;
  onHostMetrics?: (metrics: HostMetrics, topic: string) => void;
  onConnected?: () => void;
  onDisconnected?: () => void;
  onTransportMode?: (mode: ChappeTransportMode) => void;
  onError?: (message: string) => void;
};

function decodeSha256Fingerprint(b64: string): Uint8Array {
  const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
  if (bytes.length !== 32) {
    throw new Error(`expected 32-byte sha-256 fingerprint, got ${bytes.length}`);
  }
  return bytes;
}

async function fetchServerCertificateHashes(
  httpUrl: string,
): Promise<WebTransportCertificateHash[]> {
  const res = await fetch(`${httpUrl}/tls/fingerprint`);
  if (!res.ok) {
    throw new Error(`tls fingerprint failed: ${res.status}`);
  }
  const body = (await res.json()) as {
    algorithm?: string;
    value?: string;
    hashes?: { algorithm: string; value: string }[];
  };
  const entries = body.hashes?.length
    ? body.hashes
    : body.value
      ? [{ algorithm: body.algorithm ?? 'sha-256', value: body.value }]
      : [];
  if (entries.length === 0) {
    throw new Error('tls fingerprint response empty');
  }
  return entries.map((entry) => {
    if (entry.algorithm !== 'sha-256' || !entry.value) {
      throw new Error(`unsupported tls fingerprint: ${entry.algorithm}`);
    }
    return {
      algorithm: 'sha-256' as const,
      value: decodeSha256Fingerprint(entry.value),
    };
  });
}

function writeLengthPrefixed(
  writer: WritableStreamDefaultWriter<Uint8Array>,
  payload: Uint8Array,
): Promise<void> {
  const header = new Uint8Array(4);
  new DataView(header.buffer).setUint32(0, payload.length, true);
  return writer.write(header).then(() => writer.write(payload));
}

async function readLengthPrefixedFromStream(
  reader: ReadableStreamDefaultReader<Uint8Array>,
  buffer: Uint8Array[],
  bufferedLen: { value: number },
): Promise<Uint8Array | null> {
  while (true) {
    let combined = concatChunks(buffer);
    if (combined.length >= 4) {
      const frameLen = new DataView(
        combined.buffer,
        combined.byteOffset,
        Math.min(4, combined.byteLength),
      ).getUint32(0, true);
      if (frameLen === 0 || frameLen > 4 * 1024 * 1024) {
        return null;
      }
      if (combined.length >= 4 + frameLen) {
        const frame = combined.slice(4, 4 + frameLen);
        const remainder = combined.slice(4 + frameLen);
        buffer.length = 0;
        if (remainder.length > 0) {
          buffer.push(remainder);
        }
        bufferedLen.value = remainder.length;
        return frame;
      }
    }
    const chunk = await reader.read();
    if (chunk.done || !chunk.value) {
      return null;
    }
    buffer.push(chunk.value);
    bufferedLen.value += chunk.value.length;
  }
}

function concatChunks(chunks: Uint8Array[]): Uint8Array {
  if (chunks.length === 1) {
    return chunks[0]!;
  }
  const total = chunks.reduce((sum, c) => sum + c.length, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return out;
}

export function dispatchEnvelope(
  envelopeBytes: Uint8Array,
  handlers: ChappeTelemetryHandlers,
  topicHint?: string,
): void {
  const envelope = fromBinary(EnvelopeSchema, envelopeBytes);
  if (!envelope.payload.length) {
    return;
  }
  switch (envelope.messageType) {
    case 'marengo.v1.RobotState':
      handlers.onRobotState(fromBinary(RobotStateSchema, envelope.payload));
      break;
    case 'marengo.v1.SafetyState':
      handlers.onSafetyState(fromBinary(SafetyStateSchema, envelope.payload));
      break;
    case 'marengo.v1.Heartbeat':
      handlers.onHeartbeat(fromBinary(HeartbeatSchema, envelope.payload));
      break;
    case 'marengo.v1.ImuSample':
      handlers.onImuSample?.(fromBinary(ImuSampleSchema, envelope.payload));
      break;
    case 'marengo.v1.LogEvent':
      handlers.onLogEvent?.(fromBinary(LogEventSchema, envelope.payload));
      break;
    case 'marengo.v1.HostMetrics':
      handlers.onHostMetrics?.(
        fromBinary(HostMetricsSchema, envelope.payload),
        topicHint ?? '',
      );
      break;
    default:
      break;
  }
}

function webTransportAvailable(): boolean {
  return typeof WebTransport !== 'undefined';
}

async function connectWebTransport(
  handlers: ChappeTelemetryHandlers,
  closed: () => boolean,
): Promise<(() => void) | null> {
  const endpoints = getChappeEndpoints();
  if (!endpoints || !webTransportAvailable()) {
    return null;
  }

  const serverCertificateHashes = await fetchServerCertificateHashes(
    endpoints.httpUrl,
  );
  const transport = new WebTransport(endpoints.webTransportUrl, {
    allowPooling: false,
    serverCertificateHashes,
  } as WebTransportOptions);
  await transport.ready;
  if (closed()) {
    transport.close();
    return null;
  }

  handlers.onTransportMode?.('webtransport');
  handlers.onConnected?.();

  const stream = await transport.createBidirectionalStream();
  const writer = stream.writable.getWriter();
  const reader = stream.readable.getReader();

  const subscribe = create(GatewaySubscribeSchema, {
    topics: getChappeSubscribeTopics(),
  });
  await writeLengthPrefixed(writer, toBinary(GatewaySubscribeSchema, subscribe));

  const frameBuffer: Uint8Array[] = [];
  const frameBufferedLen = { value: 0 };

  void (async () => {
    while (!closed()) {
      const frame = await readLengthPrefixedFromStream(
        reader,
        frameBuffer,
        frameBufferedLen,
      );
      if (!frame) {
        break;
      }
      try {
        dispatchEnvelope(frame, handlers);
      } catch (err) {
        handlers.onError?.(err instanceof Error ? err.message : String(err));
      }
    }
    handlers.onDisconnected?.();
  })();

  return () => transport.close();
}

async function connectHttpStream(
  handlers: ChappeTelemetryHandlers,
  closed: () => boolean,
): Promise<(() => void) | null> {
  const endpoints = getChappeEndpoints();
  if (!endpoints) {
    return null;
  }

  const topics = getChappeSubscribeTopics().join(',');
  const res = await fetch(
    `${endpoints.httpUrl}/stream/chappe?topics=${encodeURIComponent(topics)}`,
  );
  if (!res.ok || !res.body) {
    throw new Error(`http stream failed: ${res.status}`);
  }
  if (closed()) {
    return null;
  }

  handlers.onTransportMode?.('http-stream');
  handlers.onConnected?.();

  const reader = res.body.getReader();
  const buffer: Uint8Array[] = [];
  const bufferedLen = { value: 0 };
  const abort = new AbortController();

  void (async () => {
    while (!closed() && !abort.signal.aborted) {
      const frame = await readLengthPrefixedFromStream(reader, buffer, bufferedLen);
      if (!frame) {
        break;
      }
      try {
        dispatchEnvelope(frame, handlers);
      } catch (err) {
        handlers.onError?.(err instanceof Error ? err.message : String(err));
      }
    }
    handlers.onDisconnected?.();
  })();

  return () => abort.abort();
}

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

  const isClosed = () => closed;

  try {
    if (webTransportAvailable()) {
      try {
        cleanup = (await connectWebTransport(handlers, isClosed)) ?? undefined;
      } catch (err) {
        handlers.onError?.(
          err instanceof Error ? err.message : String(err),
        );
      }
    }
    if (!cleanup) {
      cleanup = (await connectHttpStream(handlers, isClosed)) ?? undefined;
    }
    if (!cleanup) {
      handlers.onTransportMode?.('offline');
      handlers.onError?.('no Chappe transport available');
    }
  } catch (err) {
    handlers.onError?.(err instanceof Error ? err.message : String(err));
    handlers.onTransportMode?.('offline');
    handlers.onDisconnected?.();
  }

  return () => {
    closed = true;
    cleanup?.();
  };
}

/** @deprecated Use connectChappeStream */
export const connectChappeTelemetry = connectChappeStream;

export async function fetchGatewayHealth(): Promise<boolean> {
  const endpoints = getChappeEndpoints();
  if (!endpoints) {
    return false;
  }
  try {
    const res = await fetch(`${endpoints.httpUrl}/health`);
    if (!res.ok) {
      return false;
    }
    const body = (await res.json()) as { ok?: boolean };
    return body.ok === true;
  } catch {
    return false;
  }
}

export async function postEnableCommand(enable: boolean): Promise<void> {
  const endpoints = getChappeEndpoints();
  if (!endpoints) {
    throw new Error('Chappe endpoints not configured');
  }
  const request = create(EnableRequestSchema, {
    timestampMs: BigInt(Date.now()),
    operatorId: 'consul',
    enable,
  });
  const res = await fetch(`${endpoints.httpUrl}/command/enable`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-protobuf' },
    body: toBinary(EnableRequestSchema, request),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`enable failed: ${res.status} ${text}`);
  }
}
