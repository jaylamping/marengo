/**
 * PROTOTYPE ONLY — floating variant bar. Hidden in production builds.
 */
import { useEffect } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

export const HARDWARE_PROTO_VARIANTS = [
  { key: 'A', name: 'Table · 3D' },
  { key: 'B', name: 'Limb rail' },
  { key: 'C', name: 'Ortho board' },
] as const;

export type HardwareProtoVariantKey = (typeof HARDWARE_PROTO_VARIANTS)[number]['key'];

export function PrototypeSwitcher({
  variants = HARDWARE_PROTO_VARIANTS,
}: {
  variants?: readonly { key: string; name: string }[];
}) {
  if (import.meta.env.PROD) {
    return null;
  }

  const [params] = useSearchParams();
  const navigate = useNavigate();
  const current = params.get('variant') ?? variants[0]?.key ?? 'A';
  const idx = Math.max(
    0,
    variants.findIndex((v) => v.key === current),
  );
  const label = variants[idx] ?? variants[0];

  const go = (nextIdx: number) => {
    const wrapped = (nextIdx + variants.length) % variants.length;
    const key = variants[wrapped]?.key ?? 'A';
    const next = new URLSearchParams(params);
    next.set('variant', key);
    navigate({ search: next.toString() }, { replace: true });
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (
        t &&
        (t.tagName === 'INPUT' ||
          t.tagName === 'TEXTAREA' ||
          t.isContentEditable)
      ) {
        return;
      }
      if (e.key === 'ArrowLeft') {
        e.preventDefault();
        go(idx - 1);
      }
      if (e.key === 'ArrowRight') {
        e.preventDefault();
        go(idx + 1);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [idx, params, navigate, variants]);

  return (
    <div
      className={cn(
        'pointer-events-auto fixed bottom-5 left-1/2 z-[80] flex -translate-x-1/2 items-center gap-2',
        'rounded-full border border-amber-400/50 bg-zinc-950/95 px-2 py-1.5 text-amber-100 shadow-xl',
      )}
      data-prototype="hardware-switcher"
    >
      <Button
        type="button"
        size="sm"
        variant="ghost"
        className="h-8 px-2 text-amber-100 hover:bg-amber-400/10 hover:text-amber-50"
        onClick={() => go(idx - 1)}
        aria-label="Previous variant"
      >
        ←
      </Button>
      <div className="min-w-[10rem] px-2 text-center font-mono text-[11px] tracking-wide">
        PROTOTYPE · {label?.key} — {label?.name}
      </div>
      <Button
        type="button"
        size="sm"
        variant="ghost"
        className="h-8 px-2 text-amber-100 hover:bg-amber-400/10 hover:text-amber-50"
        onClick={() => go(idx + 1)}
        aria-label="Next variant"
      >
        →
      </Button>
    </div>
  );
}
