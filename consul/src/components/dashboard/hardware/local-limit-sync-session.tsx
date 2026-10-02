import { useState } from 'react';

import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  localLimitSyncCredential,
  setLocalLimitSyncCredential,
} from '@/lib/local-limit-sync-session';

export function LocalLimitSyncSession() {
  const [credential, setCredential] = useState(localLimitSyncCredential);
  if (!(import.meta.env.VITE_LIMIT_SYNC_URL as string | undefined)?.trim()) {
    return null;
  }
  return (
    <section className="flex flex-col gap-2">
      <Label htmlFor="local-mirror-credential">Local checkout mirror</Label>
      <Input
        id="local-mirror-credential"
        type="password"
        autoComplete="off"
        spellCheck={false}
        maxLength={4096}
        placeholder="Session credential"
        value={credential}
        onChange={(event) => {
          const value = event.target.value;
          setCredential(value);
          setLocalLimitSyncCredential(value);
        }}
      />
      <p className="text-xs text-muted-foreground">
        Paste the credential from the local mirror terminal. It stays in this
        browser tab until reload. Clear the field to disconnect the mirror.
      </p>
    </section>
  );
}
