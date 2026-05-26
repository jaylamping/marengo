import { create, fromBinary, toBinary } from '@bufbuild/protobuf';

import {
  EnableRequestSchema,
  EnvelopeSchema,
  GatewaySubscribeSchema,
  HeartbeatSchema,
  type Heartbeat,
  type RobotState,
  type SafetyState,
  RobotStateSchema,
  SafetyStateSchema,
} from '@/gen/marengo/v1/marengo_pb';
import { CHAPPE_TOPICS, getChappeEndpoints } from '@/lib/chappe-config';

type WebTransportCertificateHash = {
  algorithm: 'sha-256';
  value: Uint8Array;
};

function decodeSha256Fingerprint(b64: string): Uint8Array {
  const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
  if (bytes.length !== 32) {
    throw new Error(`expected 32-byte sha-256 fingerprint, got ${bytes.length}`);
  }
  return bytes;
}

/** Bench gateway uses a persisted self-signed cert; pin SPKI via HTTP before QUIC. */
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

export type ChappeTelemetryHandlers = {
  onRobotState: (state: RobotState) => void;
  onSafetyState: (state: SafetyState) => void;
  onHeartbeat: (heartbeat: Heartbeat) => void;
  onConnected?: () => void;
  onDisconnected?: () => void;
  onError?: (message: string) => void;
};

function writeLengthPrefixed(
  writer: WritableStreamDefaultWriter<Uint8Array>,
  payload: Uint8Array,
): Promise<void> {
  const header = new Uint8Array(4);
  new DataView(header.buffer).setUint32(0, payload.length, true);
  return writer.write(header).then(() => writer.write(payload));
}

async function readLengthPrefixed(
  reader: ReadableStreamDefaultReader<Uint8Array>,
): Promise<Uint8Array | null> {
  const lenResult = await reader.read();
  if (lenResult.done || !lenResult.value || lenResult.value.length < 4) {
    return null;
  }
  const len = new DataView(
    lenResult.value.buffer,
    lenResult.value.byteOffset,
    4,
  ).getUint32(0, true);
  if (len === 0 || len > 4 * 1024 * 1024) {
    return null;
  }
  const chunks: Uint8Array[] = [];
  let total = 0;
  while (total < len) {
    const chunk = await reader.read();
    if (chunk.done || !chunk.value) {
      return null;
    }
    chunks.push(chunk.value);
    total += chunk.value.length;
  }
  const out = new Uint8Array(len);
  let offset = 0;
  for (const c of chunks) {
    out.set(c, offset);
    offset += c.length;
  }
  return out;
}

function dispatchEnvelope(
  envelopeBytes: Uint8Array,
  handlers: ChappeTelemetryHandlers,
): void {
  const envelope = fromBinary(EnvelopeSchema, envelopeBytes);
  if (!envelope.payload.length) {
    return;
  }
  switch (envelope.messageType) {
    case 'marengo.v1.RobotState':
      handlers.onRobotState(
        fromBinary(RobotStateSchema, envelope.payload),
      );
      break;
    case 'marengo.v1.SafetyState':
      handlers.onSafetyState(
        fromBinary(SafetyStateSchema, envelope.payload),
      );
      break;
    case 'marengo.v1.Heartbeat':
      handlers.onHeartbeat(fromBinary(HeartbeatSchema, envelope.payload));
      break;
    default:
      break;
  }
}

/** Subscribe to live Chappe topics over WebTransport (binary protobuf). */
export async function connectChappeTelemetry(
  handlers: ChappeTelemetryHandlers,
): Promise<() => void> {
  const endpoints = getChappeEndpoints();
  if (!endpoints) {
    return () => {};
  }

  let closed = false;
  let transport: WebTransport | null = null;

  try {
    const serverCertificateHashes = await fetchServerCertificateHashes(
      endpoints.httpUrl,
    );
    if (import.meta.env.DEV) {
      console.info(
        '[chappe] WebTransport cert pin',
        serverCertificateHashes.map((h) => h.value.length),
      );
    }
    transport = new WebTransport(endpoints.webTransportUrl, {
      allowPooling: false,
      serverCertificateHashes,
    } as WebTransportOptions);
    await transport.ready;
    if (closed) {
      transport.close();
      return () => {};
    }
    handlers.onConnected?.();

    const stream = await transport.createBidirectionalStream();
    const writer = stream.writable.getWriter();
    const reader = stream.readable.getReader();

    const subscribe = create(GatewaySubscribeSchema, {
      topics: [
        CHAPPE_TOPICS.state,
        CHAPPE_TOPICS.safety,
        CHAPPE_TOPICS.heartbeat,
      ],
    });
    await writeLengthPrefixed(
      writer,
      toBinary(GatewaySubscribeSchema, subscribe),
    );

    void (async () => {
      while (!closed) {
        const frame = await readLengthPrefixed(reader);
        if (!frame) {
          break;
        }
        try {
          dispatchEnvelope(frame, handlers);
        } catch (err) {
          handlers.onError?.(
            err instanceof Error ? err.message : String(err),
          );
        }
      }
      handlers.onDisconnected?.();
    })();
  } catch (err) {
    handlers.onError?.(err instanceof Error ? err.message : String(err));
    handlers.onDisconnected?.();
  }

  return () => {
    closed = true;
    transport?.close();
  };
}

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
