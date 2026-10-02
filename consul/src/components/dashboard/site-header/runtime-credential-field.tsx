import { useState } from 'react';

import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useRuntimeCredential } from '@/hooks/use-runtime-credential';
import { setRuntimeCredential, type CredentialPurpose } from '@/lib/runtime-credentials';

export function RuntimeCredentialField({
  purpose,
  label,
}: { purpose: CredentialPurpose; label: string }) {
  const credential = useRuntimeCredential(purpose);
  const [error, setError] = useState('');
  const id = `runtime-credential-${purpose}`;
  return (
    <div className="flex flex-col gap-2">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        type="password"
        autoComplete="off"
        spellCheck={false}
        maxLength={4096}
        value={credential}
        onChange={(event) => {
          try {
            setRuntimeCredential(purpose, event.target.value);
            setError('');
          } catch (err) {
            setError(err instanceof Error ? err.message : 'Credential is invalid');
          }
        }}
      />
      {error ? <p role="alert" className="text-xs text-destructive">{error}</p> : null}
    </div>
  );
}
