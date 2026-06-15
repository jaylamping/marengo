/**
 * Local Chappe HTTP mock for Consul dev (no Pi / no cargo gateway).
 * Usage: node --experimental-strip-types scripts/mock-chappe-server.mts
 */
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { create, toBinary } from '@bufbuild/protobuf';

import {
  EnvelopeSchema,
  HeartbeatSchema,
  HostMetricsSchema,
  HostNodeRole,
  JointStateSchema,
  LogEventSchema,
  OperationalMode,
  RobotStateSchema,
  SafetyStateSchema,
} from '../src/gen/marengo/v1/marengo_pb.ts';

const PORT = Number(process.env.MOCK_CHAPPE_PORT ?? 8080);
const TICK_MS = Number(process.env.MOCK_CHAPPE_TICK_MS ?? 50);

function writeFrame(res: ServerResponse, payload: Uint8Array) {
  const header = Buffer.alloc(4);
  header.writeUInt32LE(payload.length, 0);
  res.write(header);
  res.write(payload);
}

function envelope(messageType: string, msgBytes: Uint8Array, ts: number): Uint8Array {
  return toBinary(
    EnvelopeSchema,
    create(EnvelopeSchema, {
      timestampMs: BigInt(ts),
      sourceNode: 'mock-chappe',
      messageType,
      payload: msgBytes,
    }),
  );
}

function handleStream(req: IncomingMessage, res: ServerResponse) {
  res.writeHead(200, {
    'Content-Type': 'application/octet-stream',
    'Cache-Control': 'no-cache',
    Connection: 'keep-alive',
  });

  let tick = 0;
  const timer = setInterval(() => {
    const ts = Date.now();
    const angle = tick * 0.02;
    const frames = [
      envelope(
        'marengo.v1.RobotState',
        toBinary(
          RobotStateSchema,
          create(RobotStateSchema, {
            timestampMs: BigInt(ts),
            joints: [
              create(JointStateSchema, {
                name: 'left_shoulder_pitch',
                position: Math.sin(angle) * 0.5,
                velocity: Math.cos(angle) * 0.1,
                effort: 0,
              }),
            ],
          }),
        ),
        ts,
      ),
      envelope(
        'marengo.v1.SafetyState',
        toBinary(
          SafetyStateSchema,
          create(SafetyStateSchema, {
            timestampMs: BigInt(ts),
            mode: OperationalMode.READY,
            hardwareEstopAsserted: false,
            softwareEstopLatched: false,
            activeFaults: [],
          }),
        ),
        ts,
      ),
      envelope(
        'marengo.v1.Heartbeat',
        toBinary(
          HeartbeatSchema,
          create(HeartbeatSchema, {
            timestampMs: BigInt(ts),
            nodeId: 'mock',
          }),
        ),
        ts,
      ),
      envelope(
        'marengo.v1.LogEvent',
        toBinary(
          LogEventSchema,
          create(LogEventSchema, {
            timestampMs: BigInt(ts),
            level: 'info',
            target: 'mock_chappe_server',
            message: `mock tick ${tick}`,
            sessionId: '',
          }),
        ),
        ts,
      ),
      envelope(
        'marengo.v1.HostMetrics',
        toBinary(
          HostMetricsSchema,
          create(HostMetricsSchema, {
            timestampMs: BigInt(ts),
            hostname: 'mock-pi',
            nodeRole: HostNodeRole.PI,
            uptimeSec: BigInt(tick),
          }),
        ),
        ts,
      ),
      envelope(
        'marengo.v1.HostMetrics',
        toBinary(
          HostMetricsSchema,
          create(HostMetricsSchema, {
            timestampMs: BigInt(ts),
            hostname: 'mock-jetson',
            nodeRole: HostNodeRole.JETSON,
            uptimeSec: BigInt(tick),
          }),
        ),
        ts,
      ),
    ];

    for (const frame of frames) {
      writeFrame(res, frame);
    }
    tick += 1;
  }, TICK_MS);

  req.on('close', () => {
    clearInterval(timer);
  });
}

const server = createServer((req, res) => {
  const url = req.url ?? '/';
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type, x-marengo-log-token');
  if (req.method === 'OPTIONS') {
    res.writeHead(204);
    res.end();
    return;
  }
  if (url.startsWith('/tls/fingerprint')) {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ algorithm: 'sha-256', value: 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=' }));
    return;
  }
  if (url.startsWith('/health')) {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ ok: true }));
    return;
  }
  if (url.startsWith('/snapshot/logs/recent')) {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ entries: [] }));
    return;
  }
  if (url.startsWith('/stream/chappe')) {
    handleStream(req, res);
    return;
  }
  res.writeHead(404);
  res.end('not found');
});

server.listen(PORT, '127.0.0.1', () => {
  console.log(`mock-chappe listening on http://127.0.0.1:${PORT} (tick ${TICK_MS}ms)`);
});
