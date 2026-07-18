import { create, toBinary } from '@bufbuild/protobuf';

import {
  EnableRequestSchema,
  HomingCompleteSchema,
  MitCommandBatchSchema,
  type MitCommandBatch,
} from '@/gen/marengo/v1/marengo_pb';
import { getChappeEndpoints } from '@/lib/chappe-config';

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

export async function postMitCommandBatch(batch: MitCommandBatch): Promise<void> {
  const endpoints = getChappeEndpoints();
  if (!endpoints) {
    throw new Error('Chappe endpoints not configured');
  }
  const res = await fetch(`${endpoints.httpUrl}/command/mit`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-protobuf' },
    body: toBinary(MitCommandBatchSchema, batch),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`mit command failed: ${res.status} ${text}`);
  }
}

export async function postTestingMitCommandBatch(batch: MitCommandBatch): Promise<void> {
  const endpoints = getChappeEndpoints();
  if (!endpoints) {
    throw new Error('Chappe endpoints not configured');
  }
  const res = await fetch(`${endpoints.httpUrl}/command/testing_mit`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-protobuf' },
    body: toBinary(MitCommandBatchSchema, batch),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`testing mit command failed: ${res.status} ${text}`);
  }
}

export async function postHomeCommand(): Promise<void> {
  const endpoints = getChappeEndpoints();
  if (!endpoints) {
    throw new Error('Chappe endpoints not configured');
  }
  const request = create(HomingCompleteSchema, {
    timestampMs: BigInt(Date.now()),
    nodeId: 'consul',
  });
  const res = await fetch(`${endpoints.httpUrl}/command/home`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-protobuf' },
    body: toBinary(HomingCompleteSchema, request),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`home command failed: ${res.status} ${text}`);
  }
}
