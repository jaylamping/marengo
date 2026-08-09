/**
 * PROTOTYPE Variant B — Limb rail: a limb is the unit of work.
 * The rail picks the focus set, the humanoid dims to match, the list is the picker.
 */
import { useMemo, useState } from 'react';

import { cn } from '@/lib/utils';

import { HumanoidViewport } from './humanoid-viewport';
import { JointSettingsSheet } from './joint-settings-sheet';
import { PROTO_JOINTS, type Limb } from './mock-hardware';
import {
  CompletenessBadge,
  ImportButton,
  JointStatusDot,
  PageTitle,
  StatusLegend,
  openImportStub,
} from './proto-chrome';

export const variantName = 'Limb rail';

const LIMBS: { id: Limb; title: string }[] = [
  { id: 'right_arm', title: 'Right arm' },
  { id: 'left_arm', title: 'Left arm' },
  { id: 'right_leg', title: 'Right leg' },
  { id: 'left_leg', title: 'Left leg' },
];

export function VariantB() {
  const [limb, setLimb] = useState<Limb>('right_arm');
  const [selectedId, setSelectedId] = useState<string | null>('right_shoulder_pitch');
  const [sheetOpen, setSheetOpen] = useState(false);

  const limbJoints = useMemo(() => PROTO_JOINTS.filter((j) => j.limb === limb), [limb]);
  const selected = PROTO_JOINTS.find((j) => j.id === selectedId) ?? null;

  const select = (id: string | null) => {
    if (id === null) return;
    setSelectedId(id);
    const joint = PROTO_JOINTS.find((j) => j.id === id);
    if (joint) setLimb(joint.limb);
    setSheetOpen(true);
  };

  return (
    <div className="flex h-[calc(100vh-9rem)] min-h-[30rem] flex-col gap-3">
      <header className="flex items-end justify-between gap-3 border-b border-line pb-3">
        <PageTitle note="limb-first · rail picks the focus set" />
        <div className="flex items-center gap-2">
          <CompletenessBadge />
          <ImportButton onImport={openImportStub} />
        </div>
      </header>

      <div className="flex gap-2 overflow-x-auto pb-1">
        {LIMBS.map((entry) => {
          const jointsInLimb = PROTO_JOINTS.filter((j) => j.limb === entry.id);
          const gaps = jointsInLimb.filter((j) => j.completenessWarn).length;
          return (
            <button
              key={entry.id}
              type="button"
              onClick={() => setLimb(entry.id)}
              className={cn(
                'min-w-[9rem] rounded-lg border px-3 py-2 text-left transition-colors',
                limb === entry.id
                  ? 'border-accent bg-surface-2'
                  : 'border-line bg-surface-1 hover:bg-surface-2',
              )}
            >
              <div className="text-sm text-foreground">{entry.title}</div>
              <div className="micro-label mt-0.5">
                {jointsInLimb.length} joints
                {gaps > 0 ? <span className="text-accent"> · {gaps} gap</span> : null}
              </div>
            </button>
          );
        })}
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 lg:grid-cols-[minmax(0,1fr)_18rem]">
        <div className="relative min-h-[18rem] overflow-hidden rounded-lg border border-line bg-surface-0">
          <HumanoidViewport
            className="absolute inset-0"
            selectedId={selectedId}
            focusLimb={limb}
            onSelect={select}
          />
          <StatusLegend className="pointer-events-none absolute bottom-3 left-3" />
        </div>

        <div className="flex min-h-0 flex-col rounded-lg border border-line bg-surface-1">
          <div className="micro-label border-b border-line px-3 py-2">
            {limb.replace('_', ' ')}
          </div>
          <div className="flex flex-col overflow-y-auto p-1">
            {limbJoints.map((joint) => (
              <button
                key={joint.id}
                type="button"
                onClick={() => select(joint.id)}
                className={cn(
                  'rounded-md px-2 py-2 text-left transition-colors',
                  selectedId === joint.id ? 'bg-surface-3' : 'hover:bg-surface-2',
                )}
              >
                <div className="flex items-center gap-2">
                  <JointStatusDot joint={joint} />
                  <span className="text-sm text-foreground">{joint.label}</span>
                </div>
                <div className="micro-label mt-0.5 pl-3.5">
                  {joint.onCan ? 'on can' : 'description only'}
                  {joint.completenessWarn ? (
                    <span className="text-accent"> · gap</span>
                  ) : null}
                </div>
              </button>
            ))}
          </div>
        </div>
      </div>

      <JointSettingsSheet joint={selected} open={sheetOpen} onOpenChange={setSheetOpen} />
    </div>
  );
}
