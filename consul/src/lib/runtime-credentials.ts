/** Operator-entered credentials live in this tab's memory, never static assets or storage. */
export type GatewayCapability =
  | 'control'
  | 'calibration'
  | 'configuration'
  | 'management'
  | 'sensitiveRead';

export type CredentialPurpose = 'operator' | GatewayCapability | 'autoLearn';

const credentials = new Map<CredentialPurpose, string>();
const listeners = new Set<() => void>();

export function runtimeCredential(purpose: CredentialPurpose): string {
  return credentials.get(purpose) ?? '';
}

export function gatewayCredential(capability: GatewayCapability): string {
  return runtimeCredential(capability) || runtimeCredential('operator');
}

export function setRuntimeCredential(purpose: CredentialPurpose, value: string): void {
  const credential = value.trim();
  if (new TextEncoder().encode(credential).length > 4096 || /[\u0000-\u001f\u007f]/.test(credential)) {
    throw new Error('Credential must be at most 4096 bytes with no control characters');
  }
  if (runtimeCredential(purpose) === credential) return;
  if (credential) credentials.set(purpose, credential);
  else credentials.delete(purpose);
  for (const listener of listeners) listener();
}

export function subscribeRuntimeCredentials(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

export function gatewayAuthHeaders(
  capability: GatewayCapability,
  contentType?: string,
): Record<string, string> {
  const headers: Record<string, string> = {};
  if (contentType) headers['Content-Type'] = contentType;
  const credential = gatewayCredential(capability);
  if (credential) headers.Authorization = `Bearer ${credential}`;
  return headers;
}
