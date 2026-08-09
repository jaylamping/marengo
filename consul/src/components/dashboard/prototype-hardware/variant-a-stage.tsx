/**
 * PROTOTYPE Variant A — Table-first Hardware.
 *
 * Default is a joint data table (shippable without 3D polish). Same click →
 * settings sheet as the stage. A header toggle reveals the Bender 3D picker.
 */
import { GridTableIcon, ThreeDViewIcon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { useMemo, useState } from 'react';

import {
  ToggleGroup,
  ToggleGroupItem,
} from '@/components/ui/toggle-group';
import { cn } from '@/lib/utils';

import { HumanoidViewport } from './humanoid-viewport';
import { JointSettingsSheet } from './joint-settings-sheet';
import { PROTO_JOINTS, type ProtoJoint } from './mock-hardware';
import {
  CompletenessBadge,
  ImportButton,
  JointStatusDot,
  PageTitle,
  StatusLegend,
  openImportStub,
} from './proto-chrome';

export const variantName = 'Table · 3D';

type ViewMode = 'table' | 'stage';

function gapFieldCount(joint: ProtoJoint): number {
  return joint.fields.filter((field) => field.warn || field.incoming).length;
}

function JointTable({
  joints,
  selectedId,
  onSelect,
}: {
  joints: ProtoJoint[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  return (
    <div className="min-h-0 flex-1 overflow-auto rounded-lg border border-line bg-surface-1">
      <table className="w-full border-collapse text-left">
        <thead className="sticky top-0 z-[1] bg-surface-2">
          <tr className="border-b border-line">
            <th className="micro-label px-3 py-2 font-medium">Status</th>
            <th className="micro-label px-3 py-2 font-medium">Joint</th>
            <th className="micro-label px-3 py-2 font-medium">Limb</th>
            <th className="micro-label px-3 py-2 font-medium">CAN</th>
            <th className="micro-label px-3 py-2 font-medium">Gaps</th>
            <th className="micro-label px-3 py-2 font-medium">Sources</th>
          </tr>
        </thead>
        <tbody>
          {joints.map((joint) => {
            const gaps = gapFieldCount(joint);
            const sources = [...new Set(joint.fields.map((field) => field.source))];
            const selected = selectedId === joint.id;
            return (
              <tr
                key={joint.id}
                className={cn(
                  'cursor-pointer border-b border-line/70 transition-colors',
                  selected ? 'bg-surface-3' : 'hover:bg-surface-2',
                )}
                onClick={() => onSelect(joint.id)}
              >
                <td className="px-3 py-2.5">
                  <JointStatusDot joint={joint} />
                </td>
                <td className="px-3 py-2.5">
                  <div className="text-sm text-foreground">{joint.label}</div>
                  <div className="micro-label mt-0.5">{joint.id}</div>
                </td>
                <td className="micro-label px-3 py-2.5">
                  {joint.limb.replace('_', ' ')}
                </td>
                <td className="data-value px-3 py-2.5 text-sm">
                  {joint.onCan ? 'on' : '—'}
                </td>
                <td className="px-3 py-2.5">
                  {gaps > 0 ? (
                    <span className="data-value text-sm text-accent">{gaps}</span>
                  ) : (
                    <span className="micro-label">0</span>
                  )}
                </td>
                <td className="micro-label px-3 py-2.5">{sources.join(' · ')}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

export function VariantA() {
  const [view, setView] = useState<ViewMode>('table');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [joints, setJoints] = useState(PROTO_JOINTS);

  const selected: ProtoJoint | null = useMemo(
    () => joints.find((j) => j.id === selectedId) ?? null,
    [joints, selectedId],
  );

  const select = (id: string | null) => {
    if (id === null) return;
    setSelectedId(id);
    setSheetOpen(true);
  };

  return (
    <div
      className={cn(
        'pointer-events-auto flex h-[calc(100vh-9rem)] min-h-[30rem] flex-col',
        view === 'stage'
          ? 'relative overflow-hidden rounded-lg border border-line bg-surface-0'
          : 'gap-3',
      )}
    >
      {view === 'stage' ? (
        <HumanoidViewport
          className="absolute inset-0"
          selectedId={selectedId}
          onSelect={select}
        />
      ) : null}

      <header
        className={cn(
          'z-10 flex items-start justify-between gap-3',
          view === 'stage'
            ? 'pointer-events-none absolute inset-x-0 top-0 p-4'
            : 'border-b border-line pb-3',
        )}
      >
        <div
          className={cn(
            view === 'stage' &&
              'pointer-events-auto rounded-lg border border-line bg-surface-0 px-3 py-2',
          )}
        >
          <PageTitle
            note={
              view === 'table'
                ? 'master · marengo.urdf · table default · click a row'
                : 'master · marengo.urdf · 3d picker · click a joint'
            }
          />
        </div>

        <div
          className={cn(
            'flex items-center gap-2',
            view === 'stage' && 'pointer-events-auto',
          )}
        >
          <ToggleGroup
            multiple={false}
            value={[view]}
            onValueChange={(value) => {
              const next = value[0] as ViewMode | undefined;
              if (next) setView(next);
            }}
            variant="outline"
            size="sm"
            className="bg-surface-0"
          >
            <ToggleGroupItem value="table" aria-label="Table view" className="gap-1.5 px-2.5">
              <HugeiconsIcon icon={GridTableIcon} size={14} />
              Table
            </ToggleGroupItem>
            <ToggleGroupItem value="stage" aria-label="3D view" className="gap-1.5 px-2.5">
              <HugeiconsIcon icon={ThreeDViewIcon} size={14} />
              3D
            </ToggleGroupItem>
          </ToggleGroup>
          <CompletenessBadge />
          <ImportButton onImport={openImportStub} />
        </div>
      </header>

      {view === 'table' ? (
        <>
          <StatusLegend />
          <JointTable joints={joints} selectedId={selectedId} onSelect={select} />
          <div className="micro-label">
            state: view=table selected={selectedId ?? 'none'} · 3d stays optional until polished
          </div>
        </>
      ) : (
        <footer className="pointer-events-none absolute inset-x-0 bottom-0 z-10 flex items-end justify-between gap-3 p-4">
          <StatusLegend className="rounded-lg border border-line bg-surface-0 px-3 py-2" />
          <span className="micro-label rounded-lg border border-line bg-surface-0 px-3 py-2">
            drag to orbit · scroll to zoom
          </span>
        </footer>
      )}

      <JointSettingsSheet
        joint={selected}
        open={sheetOpen}
        onOpenChange={(open) => {
          setSheetOpen(open);
          if (!open) setSelectedId(null);
        }}
        onAcceptIncoming={(fieldId) => {
          setJoints((prev) =>
            prev.map((joint) =>
              joint.id === selectedId
                ? {
                    ...joint,
                    fields: joint.fields.map((field) =>
                      field.id === fieldId && field.incoming
                        ? { ...field, value: field.incoming, incoming: undefined }
                        : field,
                    ),
                  }
                : joint,
            ),
          );
        }}
      />
    </div>
  );
}
