import { create, fromBinary, toBinary } from '@bufbuild/protobuf';

import {
  EnvelopeSchema,
  GatewaySubscribeSchema,
  GatewaySubscriptionAdmissionSchema,
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
  RuntimeObservationGapSchema,
  type RuntimeObservationGap,
  RuntimeConnectionStateSchema,
  type RuntimeConnectionState,
} from '@/gen/marengo/v1/marengo_pb';
import {
  getChappeEndpoints,
  getChappeSubscribeTopics,
} from '@/lib/chappe-config';
import { shouldDecodeLogEvents } from '@/lib/log-buffer';
import { gatewayAuthHeaders, gatewayCredential } from '@/lib/runtime-credentials';
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
  onRuntimeObservationGap?: (gap: RuntimeObservationGap) => void;
  onRuntimeConnectionState?: (state: RuntimeConnectionState) => void;
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
  const abort = new AbortController();
  const res = await withAdmissionDeadline(
    fetch(`${httpUrl}/tls/fingerprint`, { signal: abort.signal }),
    () => abort.abort(),
  );
  if (!res.ok) {
    throw new Error(`tls fingerprint failed: ${res.status}`);
  }
  const body = (await withAdmissionDeadline(res.json(), () => abort.abort())) as {
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
  maximum = 4 * 1024 * 1024,
): Promise<Uint8Array | null> {
  while (true) {
    let combined = concatChunks(buffer);
    if (combined.length >= 4) {
      const frameLen = new DataView(
        combined.buffer,
        combined.byteOffset,
        Math.min(4, combined.byteLength),
      ).getUint32(0, true);
      if (frameLen === 0 || frameLen > maximum) {
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
    const { value, done } = await reader.read();
    if (done || !value) {
      return null;
    }
    buffer.push(value);
    bufferedLen.value += value.length;
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
    case 'marengo.v1.RuntimeObservationGap':
      handlers.onRuntimeObservationGap?.(fromBinary(RuntimeObservationGapSchema, envelope.payload));
      break;
    case 'marengo.v1.RuntimeConnectionState':
      handlers.onRuntimeConnectionState?.(fromBinary(RuntimeConnectionStateSchema, envelope.payload));
      break;
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
      if (!shouldDecodeLogEvents()) {
        break;
      }
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

export function webTransportAvailable(): boolean {
  return typeof WebTransport !== 'undefined';
}

const ADMISSION_TIMEOUT_MS = 5000;

async function withAdmissionDeadline<T>(promise: Promise<T>, cancel: () => void): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      cancel();
      reject(new Error('Gateway subscription timed out'));
    }, ADMISSION_TIMEOUT_MS);
  });
  try {
    return await Promise.race([promise, timeout]);
  } finally {
    clearTimeout(timer);
  }
}

function subscriptionTopics(credential: string): string[] {
  return getChappeSubscribeTopics().filter((topic) => credential || topic !== 'logs/structured');
}

export async function connectWebTransport(
  handlers: ChappeTelemetryHandlers,
  closed: () => boolean,
): Promise<(() => void) | null> {
  const endpoints = getChappeEndpoints();
  if (!endpoints || !webTransportAvailable()) return null;

  const serverCertificateHashes = await fetchServerCertificateHashes(endpoints.httpUrl);
  const transport = new WebTransport(endpoints.webTransportUrl, {
    allowPooling: false,
    serverCertificateHashes,
  } as WebTransportOptions);
  // ready and closed can both reject on a failed or cancelled handshake.
  void transport.closed.catch(() => {});
  let stopped = false;
  const isClosed = () => stopped || closed();
  const stop = () => { stopped = true; transport.close(); };
  try {
    await withAdmissionDeadline(transport.ready, stop);
    if (isClosed()) { stop(); return null; }
    const stream = await withAdmissionDeadline(transport.createBidirectionalStream(), stop);
    const writer = stream.writable.getWriter();
    const reader = stream.readable.getReader();
    const credential = gatewayCredential('sensitiveRead');
    const topics = subscriptionTopics(credential);
    const subscribe = create(GatewaySubscribeSchema, { topics, runtimeCredential: credential });
    const frameBuffer: Uint8Array[] = [];
    const frameBufferedLen = { value: 0 };
    const admissionBytes = await withAdmissionDeadline((async () => {
      await writeLengthPrefixed(writer, toBinary(GatewaySubscribeSchema, subscribe));
      return readLengthPrefixedFromStream(reader, frameBuffer, frameBufferedLen, 16 * 1024);
    })(), stop);
    if (!admissionBytes) throw new Error('Gateway subscription admission missing');
    const admission = fromBinary(GatewaySubscriptionAdmissionSchema, admissionBytes);
    if (admission.status !== 200) throw new Error(`Gateway subscription refused: ${admission.status}`);
    // Every stream also carries the gateway's mandatory producer invalidation topic.
    const expectedTopics = [...new Set([...topics, 'gateway/runtime_connection'])];
    if (admission.topics.length !== expectedTopics.length || expectedTopics.some((topic) => !admission.topics.includes(topic))) {
      throw new Error('Gateway subscription topics differ from the request');
    }
    if (isClosed()) { stop(); return null; }
    handlers.onTransportMode?.('webtransport');
    handlers.onConnected?.();
    void (async () => {
      try {
        let framesSinceYield = 0;
        while (!isClosed()) {
          const frame = await readLengthPrefixedFromStream(reader, frameBuffer, frameBufferedLen);
          if (!frame || isClosed()) break;
          dispatchEnvelope(frame, handlers);
          if (++framesSinceYield >= 32) {
            framesSinceYield = 0;
            await new Promise<void>((resolve) => window.setTimeout(resolve, 0));
          }
        }
      } catch (err) {
        if (!isClosed()) handlers.onError?.(err instanceof Error ? err.message : String(err));
      } finally {
        if (!isClosed()) { handlers.onDisconnected?.(); stop(); }
      }
    })();
    return stop;
  } catch (err) {
    stop();
    throw err;
  }
}

export async function connectHttpStream(
  handlers: ChappeTelemetryHandlers,
  closed: () => boolean,
): Promise<(() => void) | null> {
  const endpoints = getChappeEndpoints();
  if (!endpoints) return null;
  const credential = gatewayCredential('sensitiveRead');
  const topics = subscriptionTopics(credential).join(',');
  const abort = new AbortController();
  let stopped = false;
  const isClosed = () => stopped || closed();
  const res = await withAdmissionDeadline(fetch(
    `${endpoints.httpUrl}/stream/chappe?topics=${encodeURIComponent(topics)}`,
    { headers: gatewayAuthHeaders('sensitiveRead'), signal: abort.signal },
  ), () => abort.abort());
  if (!res.ok || !res.body) {
    abort.abort();
    throw new Error(`http stream failed: ${res.status}`);
  }
  if (isClosed()) { abort.abort(); await res.body.cancel(); return null; }
  handlers.onTransportMode?.('http-stream');
  handlers.onConnected?.();
  const reader = res.body.getReader();
  const buffer: Uint8Array[] = [];
  const bufferedLen = { value: 0 };
  const stop = () => {
    stopped = true;
    abort.abort();
    void reader.cancel().catch(() => {});
  };
  void (async () => {
    try {
      let framesSinceYield = 0;
      while (!isClosed()) {
        const frame = await readLengthPrefixedFromStream(reader, buffer, bufferedLen);
        if (!frame || isClosed()) break;
        dispatchEnvelope(frame, handlers);
        if (++framesSinceYield >= 32) {
          framesSinceYield = 0;
          await new Promise<void>((resolve) => window.setTimeout(resolve, 0));
        }
      }
    } catch (err) {
      if (!isClosed()) handlers.onError?.(err instanceof Error ? err.message : String(err));
    } finally {
      if (!isClosed()) { handlers.onDisconnected?.(); stop(); }
    }
  })();
  return stop;
}
