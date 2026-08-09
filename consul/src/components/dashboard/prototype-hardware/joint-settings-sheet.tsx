/**
 * PROTOTYPE — unified mapped-field sheet (shared content shape only).
 */
import { Button } from '@/components/ui/button';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { cn } from '@/lib/utils';

import type { MappedField, ProtoJoint } from './mock-hardware';

const SOURCE_STYLE: Record<MappedField['source'], string> = {
  urdf: 'text-sky-300/90',
  'motors.yaml': 'text-violet-300/90',
  'control.yaml': 'text-emerald-300/90',
  'homing.yaml': 'text-amber-300/90',
};

export function JointSettingsSheet({
  joint,
  open,
  onOpenChange,
  onAcceptIncoming,
}: {
  joint: ProtoJoint | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAcceptIncoming?: (fieldId: string) => void;
}) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="right"
        variant="panel"
        className="w-full sm:max-w-md"
        showOverlay
      >
        {joint ? (
          <>
            <SheetHeader className="border-b border-line">
              <SheetTitle className="font-sans text-base tracking-tight">
                {joint.label}
              </SheetTitle>
              <SheetDescription className="font-mono text-[11px] text-muted-foreground">
                {joint.id} · {joint.onCan ? 'on CAN' : 'description only'} ·
                sources tagged per field
              </SheetDescription>
            </SheetHeader>
            <div className="flex-1 space-y-3 overflow-y-auto px-6 py-4">
              {joint.fields.map((f) => (
                <div
                  key={f.id}
                  className={cn(
                    'grid grid-cols-[1fr_auto] gap-x-3 gap-y-1 border-b border-line/60 py-2',
                    f.warn && 'bg-orange-500/5',
                  )}
                >
                  <div className="min-w-0">
                    <div className="flex items-baseline gap-2">
                      <span className="text-sm text-foreground">{f.label}</span>
                      {f.warn ? (
                        <span className="font-mono text-[10px] text-orange-400">
                          warn
                        </span>
                      ) : null}
                    </div>
                    <div
                      className={cn(
                        'font-mono text-[10px]',
                        SOURCE_STYLE[f.source],
                      )}
                    >
                      {f.source}
                    </div>
                  </div>
                  <div className="text-right font-mono text-sm tabular-nums">
                    {f.value}
                    {f.incoming ? (
                      <button
                        type="button"
                        className="mt-1 block text-[11px] text-amber-300 underline-offset-2 hover:underline"
                        onClick={() => onAcceptIncoming?.(f.id)}
                      >
                        incoming {f.incoming} → accept
                      </button>
                    ) : null}
                  </div>
                </div>
              ))}
            </div>
            <SheetFooter className="border-t border-line">
              <Button type="button" variant="secondary" className="w-full">
                Accept all incoming (stub)
              </Button>
            </SheetFooter>
          </>
        ) : null}
      </SheetContent>
    </Sheet>
  );
}
