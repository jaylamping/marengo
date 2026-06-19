import { create, fromBinary, toBinary } from '@bufbuild/protobuf';

import type {
  ActuatorLimitSnapshot,
  OperatorCommand,
} from '@/gen/marengo/v1/marengo_pb';
import {
  ActuatorLimitSnapshotSchema,
  EnvelopeSchema,
  OperatorCommandSchema,
  SessionStartResponseSchema,
} from '@/gen/marengo/v1/marengo_pb';
import { getChappeEndpoints } from '@/lib/chappe-config';

function requireEndpoints() {
  const endpoints = getChappeEndpoints();
  if (!endpoints) {
    throw new Error('Chappe endpoints not configured');
  }
  return endpoints;
}

export async function startActuatorSession(
  operatorId = 'consul',
): Promise<{ sessionId: string; startedMs: bigint }> {
  void operatorId;
  const { httpUrl } = requireEndpoints();
  const res = await fetch(`${httpUrl}/session/start`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-protobuf' },
    body: new Uint8Array(),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`session start failed: ${res.status} ${text}`);
  }
  const bytes = new Uint8Array(await res.arrayBuffer());
  const parsed = fromBinary(SessionStartResponseSchema, bytes);
  return { sessionId: parsed.sessionId, startedMs: parsed.startedMs };
}

export async function fetchActuatorLimits(): Promise<ActuatorLimitSnapshot | null> {
  const { httpUrl } = requireEndpoints();
  const res = await fetch(`${httpUrl}/snapshot/actuator/limits`);
  if (res.status === 503) {
    return null;
  }
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`actuator limits failed: ${res.status} ${text}`);
  }
  const bytes = new Uint8Array(await res.arrayBuffer());
  return fromBinary(ActuatorLimitSnapshotSchema, bytes);
}

export async function postActuatorCommand(command: OperatorCommand): Promise<void> {
  const { httpUrl } = requireEndpoints();
  const envelope = create(EnvelopeSchema, {
    timestampMs: command.timestampMs,
    sourceNode: 'consul',
    messageType: 'marengo.v1.OperatorCommand',
    payload: toBinary(OperatorCommandSchema, command),
  });
  const res = await fetch(`${httpUrl}/command/actuator`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-protobuf' },
    body: toBinary(EnvelopeSchema, envelope),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`actuator command failed: ${res.status} ${text}`);
  }
}
