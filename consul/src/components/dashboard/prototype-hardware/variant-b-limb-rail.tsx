/**
 * PROTOTYPE Variant B — Limb rail: horizontal limb strips drive focus; 3D is secondary.
 */
import { Alert02Icon, Upload01Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { useMemo, useState } from 'react';

import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

import { JointSettingsSheet } from './joint-settings-sheet';
import { PROTO_JOINTS, WARN_COUNT, type ProtoJoint } from './mock-hardware';
import { StickHumanoidCanvas } from './stick-humanoid';

export const variantName = 'Limb rail';

const LIMBS: { id: ProtoJoint['limb']; title: string }[] = [
  { id: 'right_arm', title: 'Right arm' },
  { id: 'left_arm', title: 'Left arm' },
  { id: 'right_leg', title: 'Right leg' },
  { id: 'left_leg', title: 'Left leg' },
];

export function VariantB() {
  const [limb, setLimb] = useState<ProtoJoint['limb']>('right_arm');
  const [selectedId, setSelectedId] = useState<string | null>(
    'right_shoulder_pitch',
  );
  const [sheetOpen, setSheetOpen] = useState(false);

  const limbJoints = useMemo(
    () => PROTO_JOINTS.filter((j) => j.limb === limb),
    [limb],
  );
  const selected = PROTO_JOINTS.find((j) => j.id === selectedId) ?? null;

  return (
    <div className="flex h-[calc(100vh-5.5rem)] min-h-[28rem] flex-col gap-3">
      <header className="flex items-end justify-between gap-3 border-b border-line pb-3">
        <div>
          <h1 className="font-sans text-lg tracking-tight">Hardware</h1>
          <p className="font-mono text-[11px] text-muted-foreground">
            limb-first · master YAML · rail picks the focus set
          </p>
        </div>
        <div className="flex items-center gap-2">
          <span className="inline-flex h-8 items-center gap-1 rounded-md border border-line px-2 font-mono text-[11px] text-orange-400">
            <HugeiconsIcon icon={Alert02Icon} size={14} />
            {WARN_COUNT}
          </span>
          <Button type="button" size="sm" variant="secondary" className="gap-1.5">
            <HugeiconsIcon icon={Upload01Icon} size={16} />
            Import
          </Button>
        </div>
      </header>

      <div className="flex gap-2 overflow-x-auto pb-1">
        {LIMBS.map((l) => (
          <button
            key={l.id}
            type="button"
            onClick={() => setLimb(l.id)}
            className={cn(
              'min-w-[8.5rem] rounded-md border px-3 py-2 text-left transition-colors',
              limb === l.id
                ? 'border-amber-400/60 bg-amber-400/10'
                : 'border-line bg-surface-1 hover:bg-surface-2',
            )}
          >
            <div className="text-sm">{l.title}</div>
            <div className="font-mono text-[10px] text-muted-foreground">
              {PROTO_JOINTS.filter((j) => j.limb === l.id).length} joints
            </div>
          </button>
        ))}
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 lg:grid-cols-[minmax(0,1fr)_16rem]">
        <div className="relative min-h-[16rem] overflow-hidden rounded-lg border border-line">
          <StickHumanoidCanvas
            className="absolute inset-0 opacity-90"
            selectedId={selectedId}
            onSelect={(id) => {
              setSelectedId(id);
              const j = PROTO_JOINTS.find((x) => x.id === id);
              if (j) setLimb(j.limb);
              setSheetOpen(true);
            }}
          />
        </div>
        <div className="flex flex-col gap-1 overflow-y-auto rounded-lg border border-line bg-surface-1 p-2">
          <div className="px-1 pb-1 font-mono text-[10px] tracking-wide text-muted-foreground uppercase">
            {limb.replace('_', ' ')}
          </div>
          {limbJoints.map((j) => (
            <button
              key={j.id}
              type="button"
              onClick={() => {
                setSelectedId(j.id);
                setSheetOpen(true);
              }}
              className={cn(
                'rounded-md px-2 py-2 text-left text-sm',
                selectedId === j.id
                  ? 'bg-amber-400/15 text-foreground'
                  : 'hover:bg-surface-2',
              )}
            >
              <div className="flex items-center justify-between gap-2">
                <span>{j.label}</span>
                {j.completenessWarn ? (
                  <span className="font-mono text-[10px] text-orange-400">!</span>
                ) : null}
              </div>
              <div className="font-mono text-[10px] text-muted-foreground">
                {j.onCan ? 'on CAN' : 'off wire'}
              </div>
            </button>
          ))}
        </div>
      </div>

      <div className="font-mono text-[10px] text-muted-foreground">
        state: limb={limb} selected={selectedId ?? 'none'} sheet={String(sheetOpen)}
      </div>

      <JointSettingsSheet
        joint={selected}
        open={sheetOpen}
        onOpenChange={setSheetOpen}
      />
    </div>
  );
}
