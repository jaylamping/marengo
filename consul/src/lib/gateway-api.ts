import { create, fromBinary, toBinary } from '@bufbuild/protobuf';

import type {
  ActuatorLimitSnapshot,
  MitCommandBatch,
  OperatorCommand,
} from '@/gen/marengo/v1/marengo_pb';
import {
  ActuatorLimitSnapshotSchema,
  EnableRequestSchema,
  EnvelopeSchema,
  MitCommandBatchSchema,
  OperatorCommandSchema,
} from '@/gen/marengo/v1/marengo_pb';
import { getChappeEndpoints } from '@/lib/chappe-config';
import type {
  CommissioningScopeResponse,
  PutCommissioningScopeBody,
} from '@/lib/commissioning-scope';

export type { CommissioningScopeResponse, PutCommissioningScopeBody };

function requireEndpoints() {
  const endpoints = getChappeEndpoints();
  if (!endpoints) {
    throw new Error('Chappe endpoints not configured');
  }
  return endpoints;
}

function authHeaders(json = true): Record<string, string> {
  const headers: Record<string, string> = {};
  if (json) {
    headers['Content-Type'] = 'application/json';
  }
  const token = import.meta.env.VITE_MARENGO_LOG_TOKEN as string | undefined;
  if (token?.trim()) {
    headers['x-marengo-log-token'] = token.trim();
  }
  return headers;
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
  const headers: Record<string, string> = {
    'Content-Type': 'application/x-protobuf',
  };
  const token = import.meta.env.VITE_MARENGO_LOG_TOKEN as string | undefined;
  if (token?.trim()) {
    headers['x-marengo-log-token'] = token.trim();
  }
  const res = await fetch(`${httpUrl}/command/actuator`, {
    method: 'POST',
    headers,
    body: toBinary(EnvelopeSchema, envelope),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`actuator command failed: ${res.status} ${text}`);
  }
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

/** @deprecated Operator HomingComplete retired — gateway returns 410 Gone. */
export async function postHomeCommand(): Promise<void> {
  throw new Error(
    'POST /command/home retired; use Hardware Set Zero per joint (not Testing Home)',
  );
}

export async function fetchCommissioningScope(): Promise<CommissioningScopeResponse> {
  const { httpUrl } = requireEndpoints();
  const res = await fetch(`${httpUrl}/hardware/commissioning-scope`, {
    headers: authHeaders(false),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`commissioning scope GET failed: ${res.status} ${text}`);
  }
  return (await res.json()) as CommissioningScopeResponse;
}

export async function putCommissioningScope(
  body: PutCommissioningScopeBody,
): Promise<CommissioningScopeResponse> {
  const { httpUrl } = requireEndpoints();
  const res = await fetch(`${httpUrl}/hardware/commissioning-scope`, {
    method: 'PUT',
    headers: authHeaders(true),
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`commissioning scope PUT failed: ${res.status} ${text}`);
  }
  return (await res.json()) as CommissioningScopeResponse;
}

export async function deleteCommissioningScope(): Promise<CommissioningScopeResponse> {
  const { httpUrl } = requireEndpoints();
  const res = await fetch(`${httpUrl}/hardware/commissioning-scope`, {
    method: 'DELETE',
    headers: authHeaders(false),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`commissioning scope DELETE failed: ${res.status} ${text}`);
  }
  return (await res.json()) as CommissioningScopeResponse;
}

/** Firmware SetZero for one joint via gateway → Pi (briefly enables, then disables). */
export async function postSetZeroCommand(
  joint: string,
  options: { signTestPassed: boolean },
): Promise<void> {
  const endpoints = getChappeEndpoints();
  if (!endpoints) {
    throw new Error('Chappe endpoints not configured');
  }
  const res = await fetch(`${endpoints.httpUrl}/command/set_zero`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      joint,
      confirm: true,
      sign_test_passed: options.signTestPassed,
      client_id: 'consul',
    }),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`set-zero failed: ${res.status} ${text}`);
  }
}

/** Light Hardware-page status solicit (Disable type-4 → OperationStatus). ~1/2s gateway cap. */
export async function postMotorStatusPoll(): Promise<void> {
  const endpoints = getChappeEndpoints();
  if (!endpoints) {
    throw new Error('Chappe endpoints not configured');
  }
  const res = await fetch(`${endpoints.httpUrl}/command/motor_status_poll`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      client_id: 'consul',
    }),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`motor status poll failed: ${res.status} ${text}`);
  }
}

export type ActiveReportingLeaseActionName = 'acquire' | 'renew' | 'release';

/** Per-joint type-24 Active Reporting lease via gateway → Pi (fire-and-forget ACK). */
export async function postActiveReportingLease(options: {
  joint: string;
  clientId: string;
  leaseId: string;
  action: ActiveReportingLeaseActionName;
  /** Prefer true for pagehide / unload release so the request outlives the page. */
  keepalive?: boolean;
}): Promise<void> {
  const endpoints = getChappeEndpoints();
  if (!endpoints) {
    throw new Error('Chappe endpoints not configured');
  }
  const res = await fetch(`${endpoints.httpUrl}/command/active_reporting_lease`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      joint: options.joint,
      client_id: options.clientId,
      lease_id: options.leaseId,
      action: options.action,
    }),
    keepalive: options.keepalive === true,
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`active-reporting lease ${options.action} failed: ${res.status} ${text}`);
  }
}
