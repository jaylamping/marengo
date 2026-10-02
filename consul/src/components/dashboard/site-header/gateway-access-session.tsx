import { useState } from 'react';

import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription, SheetTrigger } from '@/components/ui/sheet';
import { RuntimeCredentialField } from '@/components/dashboard/site-header/runtime-credential-field';
import { isChappeLive } from '@/lib/chappe-config';
import { setRuntimeCredential, type CredentialPurpose } from '@/lib/runtime-credentials';

const PURPOSES = [
  ['operator', 'Operator'],
  ['control', 'Control'],
  ['calibration', 'Calibration'],
  ['configuration', 'Configuration'],
  ['management', 'Management'],
  ['sensitiveRead', 'Logs and audit'],
] as const;

export function GatewayAccessSession() {
  const [purpose, setPurpose] = useState<CredentialPurpose>('operator');
  if (!isChappeLive()) return null;
  return (
    <Sheet>
      <SheetTrigger asChild>
        <Button variant="outline" size="sm">Robot access</Button>
      </SheetTrigger>
      <SheetContent variant="panel" className="flex flex-col gap-3 p-6">
        <SheetHeader className="p-0">
          <SheetTitle>Robot access</SheetTitle>
          <SheetDescription>Credentials for this browser tab</SheetDescription>
        </SheetHeader>
        <Label htmlFor="gateway-credential-purpose">Credential role</Label>
        <select
          id="gateway-credential-purpose"
          className="rounded-md border border-input bg-background p-2 text-sm"
          value={purpose}
          onChange={(event) => setPurpose(event.target.value as CredentialPurpose)}
        >
          {PURPOSES.map(([value, label]) => <option key={value} value={value}>{label}</option>)}
        </select>
        <RuntimeCredentialField key={purpose} purpose={purpose} label="Gateway credential" />
        <p className="text-xs text-muted-foreground">
          Enter the matching credential from your robot configuration. It stays
          in this tab until reload. The gateway determines which actions it allows.
        </p>
        <Button variant="outline" size="sm" onClick={() => setRuntimeCredential(purpose, '')}>
          Clear this credential
        </Button>
      </SheetContent>
    </Sheet>
  );
}
