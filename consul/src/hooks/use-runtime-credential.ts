import { useSyncExternalStore } from 'react';

import {
  gatewayCredential,
  runtimeCredential,
  subscribeRuntimeCredentials,
  type CredentialPurpose,
  type GatewayCapability,
} from '@/lib/runtime-credentials';

export function useRuntimeCredential(purpose: CredentialPurpose): string {
  return useSyncExternalStore(
    subscribeRuntimeCredentials,
    () => runtimeCredential(purpose),
    () => '',
  );
}

export function useGatewayCredential(capability: GatewayCapability): string {
  return useSyncExternalStore(
    subscribeRuntimeCredentials,
    () => gatewayCredential(capability),
    () => '',
  );
}
