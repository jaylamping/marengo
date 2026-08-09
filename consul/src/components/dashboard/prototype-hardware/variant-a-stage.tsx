/**
 * PROTOTYPE Variant A — Stage: full-bleed 3D is the page.
 * Question line: "Three variants of Hardware, ?variant= on /prototype/hardware."
 */
import { Alert02Icon, Upload01Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { useMemo, useState } from 'react';

import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';

import { JointSettingsSheet } from './joint-settings-sheet';
import { PROTO_JOINTS, WARN_COUNT, type ProtoJoint } from './mock-hardware';
import { StickHumanoidCanvas } from './stick-humanoid';

export const variantName = 'Stage';

export function VariantA() {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [joints, setJoints] = useState(PROTO_JOINTS);

  const selected: ProtoJoint | null = useMemo(
    () => joints.find((j) => j.id === selectedId) ?? null,
    [joints, selectedId],
  );

  const select = (id: string) => {
    setSelectedId(id);
    setSheetOpen(true);
  };

  return (
    <div className="relative flex h-[calc(100vh-5.5rem)] min-h-[28rem] flex-col overflow-hidden rounded-lg border border-line bg-surface-0">
      <header className="absolute top-0 right-0 left-0 z-10 flex items-center justify-between gap-3 px-4 py-3">
        <div>
          <h1 className="font-sans text-lg tracking-tight text-foreground">
            Hardware
          </h1>
          <p className="font-mono text-[11px] text-muted-foreground">
            master · marengo.urdf · click a joint
          </p>
        </div>
        <div className="flex items-center gap-2">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <button
                  type="button"
                  className="inline-flex h-9 w-9 items-center justify-center rounded-md border border-line bg-surface-1/80 text-orange-400 backdrop-blur"
                  aria-label={`${WARN_COUNT} completeness warnings`}
                >
                  <HugeiconsIcon icon={Alert02Icon} size={18} />
                </button>
              </TooltipTrigger>
              <TooltipContent>
                {WARN_COUNT} completeness gaps (warn only)
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            className="gap-1.5"
            onClick={() => {
              // stub import
              window.alert(
                'PROTOTYPE: Import / drop would open resolve wizard',
              );
            }}
          >
            <HugeiconsIcon icon={Upload01Icon} size={16} />
            Import
          </Button>
        </div>
      </header>

      <StickHumanoidCanvas
        className="absolute inset-0"
        selectedId={selectedId}
        onSelect={select}
      />

      <div className="pointer-events-none absolute right-4 bottom-4 left-4 flex justify-between font-mono text-[10px] text-muted-foreground">
        <span>teal = on CAN · slate = description · amber = selected · orange orb = warn</span>
        <span>state: selected={selectedId ?? 'none'}</span>
      </div>

      <JointSettingsSheet
        joint={selected}
        open={sheetOpen}
        onOpenChange={setSheetOpen}
        onAcceptIncoming={(fieldId) => {
          setJoints((prev) =>
            prev.map((j) => {
              if (j.id !== selectedId) return j;
              return {
                ...j,
                fields: j.fields.map((f) =>
                  f.id === fieldId && f.incoming
                    ? { ...f, value: f.incoming, incoming: undefined }
                    : f,
                ),
              };
            }),
          );
        }}
      />
    </div>
  );
}
