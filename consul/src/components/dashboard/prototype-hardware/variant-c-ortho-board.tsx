/**
 * PROTOTYPE Variant C — Ortho board: flat schematic board, no orbit hero.
 * Joints as a diagram you click; 3D presence is deliberately absent.
 */
import { Alert02Icon, Upload01Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { useMemo, useState } from 'react';

import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

import { JointSettingsSheet } from './joint-settings-sheet';
import { PROTO_JOINTS, WARN_COUNT } from './mock-hardware';

export const variantName = 'Ortho board';

export function VariantC() {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [dragOver, setDragOver] = useState(false);

  const selected = useMemo(
    () => PROTO_JOINTS.find((j) => j.id === selectedId) ?? null,
    [selectedId],
  );

  // project body-space → 2D board
  const nodes = PROTO_JOINTS.map((j) => ({
    ...j,
    x: 50 + j.pos[0] * 55,
    y: 18 + (1.7 - j.pos[1]) * 48,
  }));

  return (
    <div className="flex h-[calc(100vh-5.5rem)] min-h-[28rem] flex-col gap-3">
      <header
        className={cn(
          'flex items-center justify-between gap-3 rounded-lg border px-4 py-3 transition-colors',
          dragOver
            ? 'border-amber-400 bg-amber-400/10'
            : 'border-dashed border-line bg-surface-1/40',
        )}
        onDragOver={(e) => {
          e.preventDefault();
          setDragOver(true);
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={(e) => {
          e.preventDefault();
          setDragOver(false);
          window.alert(
            `PROTOTYPE: dropped ${e.dataTransfer.files[0]?.name ?? 'file'} → resolve wizard`,
          );
        }}
      >
        <div>
          <h1 className="font-sans text-lg tracking-tight">Hardware</h1>
          <p className="font-mono text-[11px] text-muted-foreground">
            ortho schematic · drop URDF/YAML on this header
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            className="inline-flex h-9 w-9 items-center justify-center rounded-full border border-line text-orange-400"
            aria-label="Completeness"
          >
            <HugeiconsIcon icon={Alert02Icon} size={18} />
          </button>
          <Button type="button" size="sm" variant="secondary" className="gap-1.5">
            <HugeiconsIcon icon={Upload01Icon} size={16} />
            Import
          </Button>
        </div>
      </header>

      <div className="relative min-h-0 flex-1 overflow-hidden rounded-lg border border-line bg-[radial-gradient(ellipse_at_center,#1a2332_0%,#0b0f14_70%)]">
        {/* torso silhouette */}
        <div className="pointer-events-none absolute top-[22%] left-1/2 h-[38%] w-[12%] -translate-x-1/2 rounded-2xl border border-slate-600/50 bg-slate-800/30" />
        <div className="pointer-events-none absolute top-[12%] left-1/2 h-[8%] w-[8%] -translate-x-1/2 rounded-full border border-slate-600/50 bg-slate-800/30" />

        <svg
          className="pointer-events-none absolute inset-0 z-0 h-full w-full"
          viewBox="0 0 100 100"
          preserveAspectRatio="xMidYMid meet"
        >
          {/* simple bone lines */}
          <line x1="50" y1="28" x2="68" y2="32" stroke="#475569" strokeWidth="0.4" />
          <line x1="68" y1="32" x2="78" y2="48" stroke="#475569" strokeWidth="0.4" />
          <line x1="50" y1="28" x2="32" y2="32" stroke="#475569" strokeWidth="0.4" />
          <line x1="32" y1="32" x2="22" y2="48" stroke="#475569" strokeWidth="0.4" />
          <line x1="50" y1="55" x2="58" y2="78" stroke="#475569" strokeWidth="0.4" />
          <line x1="50" y1="55" x2="42" y2="78" stroke="#475569" strokeWidth="0.4" />
        </svg>

        {nodes.map((n) => (
          <button
            key={n.id}
            type="button"
            title={n.label}
            onClick={() => {
              setSelectedId(n.id);
              setSheetOpen(true);
            }}
            className={cn(
              'absolute z-10 h-4 w-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 transition-transform',
              selectedId === n.id && 'scale-125 border-amber-300 bg-amber-400',
              selectedId !== n.id &&
                n.completenessWarn &&
                'border-orange-400 bg-orange-500/80',
              selectedId !== n.id &&
                !n.completenessWarn &&
                n.onCan &&
                'border-teal-300 bg-teal-400/90',
              selectedId !== n.id &&
                !n.completenessWarn &&
                !n.onCan &&
                'border-slate-400 bg-slate-500/80',
            )}
            style={{ left: `${n.x}%`, top: `${n.y}%` }}
          />
        ))}

        <div className="absolute right-3 bottom-3 left-3 flex flex-wrap gap-3 font-mono text-[10px] text-muted-foreground">
          {PROTO_JOINTS.filter((j) => j.completenessWarn).map((j) => (
            <span key={j.id} className="text-orange-400/90">
              ! {j.label}
            </span>
          ))}
        </div>
      </div>

      <div className="font-mono text-[10px] text-muted-foreground">
        state: selected={selectedId ?? 'none'} warns={WARN_COUNT} · no WebGL in this variant
      </div>

      <JointSettingsSheet
        joint={selected}
        open={sheetOpen}
        onOpenChange={setSheetOpen}
      />
    </div>
  );
}
