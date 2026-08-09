/**
 * PROTOTYPE Variant C — Ortho board: locked orthographic elevation, no orbit.
 * Still Three.js, but the camera never moves: the rig reads as a schematic and
 * the header is the drop target for incoming URDF/YAML.
 */
import { useMemo, useState } from 'react';

import { cn } from '@/lib/utils';

import { HumanoidViewport } from './humanoid-viewport';
import { JointSettingsSheet } from './joint-settings-sheet';
import { PROTO_JOINTS } from './mock-hardware';
import {
  CompletenessBadge,
  ImportButton,
  JointStatusDot,
  PageTitle,
  StatusLegend,
  openImportStub,
} from './proto-chrome';

export const variantName = 'Ortho board';

export function VariantC() {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [dragOver, setDragOver] = useState(false);

  const selected = useMemo(
    () => PROTO_JOINTS.find((j) => j.id === selectedId) ?? null,
    [selectedId],
  );
  const gaps = PROTO_JOINTS.filter((j) => j.completenessWarn);

  return (
    <div className="flex h-[calc(100vh-9rem)] min-h-[30rem] flex-col gap-3 pointer-events-auto">
      <header
        className={cn(
          'flex items-center justify-between gap-3 rounded-lg border border-dashed px-4 py-3 transition-colors',
          dragOver ? 'border-accent bg-surface-2' : 'border-line bg-surface-1',
        )}
        onDragOver={(event) => {
          event.preventDefault();
          setDragOver(true);
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={(event) => {
          event.preventDefault();
          setDragOver(false);
          window.alert(
            `PROTOTYPE — dropped ${event.dataTransfer.files[0]?.name ?? 'file'} → resolve wizard`,
          );
        }}
      >
        <PageTitle note="ortho elevation · drop urdf/yaml on this header" />
        <div className="flex items-center gap-2">
          <CompletenessBadge />
          <ImportButton onImport={openImportStub} />
        </div>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 lg:grid-cols-[minmax(0,1fr)_18rem]">
        <div className="relative min-h-[18rem] overflow-hidden rounded-lg border border-line bg-surface-0">
          <HumanoidViewport
            className="absolute inset-0"
            selectedId={selectedId}
            style="schematic"
            orbit={false}
            ground={false}
            onSelect={(id) => {
              setSelectedId(id);
              setSheetOpen(id !== null);
            }}
          />
          <span className="micro-label pointer-events-none absolute top-3 left-3">
            front elevation · camera locked
          </span>
          <StatusLegend className="pointer-events-none absolute bottom-3 left-3" />
        </div>

        <div className="flex min-h-0 flex-col rounded-lg border border-line bg-surface-1">
          <div className="micro-label border-b border-line px-3 py-2">
            completeness gaps · {gaps.length}
          </div>
          <div className="flex flex-col overflow-y-auto p-1">
            {gaps.map((joint) => (
              <button
                key={joint.id}
                type="button"
                onClick={() => {
                  setSelectedId(joint.id);
                  setSheetOpen(true);
                }}
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
                  {joint.fields.filter((f) => f.warn || f.incoming).length} fields need review
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
