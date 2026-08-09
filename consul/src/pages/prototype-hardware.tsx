/**
 * PROTOTYPE ROUTE — /prototype/hardware?variant=A|B|C
 * Throwaway UI for Wayfinder #100. Do not treat as production Hardware.
 */
import { useSearchParams } from 'react-router-dom';

import { PrototypeSwitcher } from '@/components/dashboard/prototype-hardware/prototype-switcher';
import { VariantA } from '@/components/dashboard/prototype-hardware/variant-a-stage';
import { VariantB } from '@/components/dashboard/prototype-hardware/variant-b-limb-rail';
import { VariantC } from '@/components/dashboard/prototype-hardware/variant-c-ortho-board';

export function PrototypeHardwarePage() {
  const [params] = useSearchParams();
  const variant = (params.get('variant') ?? 'A').toUpperCase();

  return (
    <div className="relative px-1 pb-16 pointer-events-auto" data-prototype="hardware-page">
      <div className="mb-2 rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 font-mono text-[11px] text-amber-100/90">
        PROTOTYPE — Hardware UI for issue #100. Not production. A is table-first with an
        optional 3D toggle (preferred direction). ← → still flips A/B/C.
      </div>
      {variant === 'B' ? <VariantB /> : null}
      {variant === 'C' ? <VariantC /> : null}
      {variant !== 'B' && variant !== 'C' ? <VariantA /> : null}
      <PrototypeSwitcher />
    </div>
  );
}
