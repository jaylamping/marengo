/**
 * PROTOTYPE Variant A — Stage: the humanoid is the page.
 * Chrome floats over a full-bleed picker; everything else lives in the sheet.
 */
import { useMemo, useState } from 'react';

import { HumanoidViewport } from './humanoid-viewport';
import { JointSettingsSheet } from './joint-settings-sheet';
import { PROTO_JOINTS, type ProtoJoint } from './mock-hardware';
import {
  CompletenessBadge,
  ImportButton,
  PageTitle,
  StatusLegend,
  openImportStub,
} from './proto-chrome';

export const variantName = 'Stage';

export function VariantA() {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [joints, setJoints] = useState(PROTO_JOINTS);

  const selected: ProtoJoint | null = useMemo(
    () => joints.find((j) => j.id === selectedId) ?? null,
    [joints, selectedId],
  );

  return (
    <div className="relative flex h-[calc(100vh-9rem)] min-h-[30rem] flex-col overflow-hidden rounded-lg border border-line bg-surface-0 pointer-events-auto">
      <HumanoidViewport
        className="absolute inset-0"
        selectedId={selectedId}
        onSelect={(id) => {
          setSelectedId(id);
          setSheetOpen(id !== null);
        }}
      />

      <header className="pointer-events-none absolute inset-x-0 top-0 z-10 flex items-start justify-between gap-3 p-4">
        <div className="pointer-events-auto rounded-lg border border-line bg-surface-0 px-3 py-2">
          <PageTitle note="master · marengo.urdf · click a joint" />
        </div>
        <div className="pointer-events-auto flex items-center gap-2">
          <CompletenessBadge />
          <ImportButton onImport={openImportStub} />
        </div>
      </header>

      <footer className="pointer-events-none absolute inset-x-0 bottom-0 z-10 flex items-end justify-between gap-3 p-4">
        <StatusLegend className="rounded-lg border border-line bg-surface-0 px-3 py-2" />
        <span className="micro-label rounded-lg border border-line bg-surface-0 px-3 py-2">
          drag to orbit · scroll to zoom
        </span>
      </footer>

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
