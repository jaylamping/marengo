/**
 * PROTOTYPE — unified mapped-field sheet: one joint, every source, source tags
 * per field. Colour is scarce on purpose: amber only where a decision is owed.
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

import type { ProtoJoint } from './mock-hardware';

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
  const incomingCount = joint?.fields.filter((field) => field.incoming).length ?? 0;

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" variant="panel" className="w-full sm:max-w-md" showOverlay>
        {joint ? (
          <>
            <SheetHeader className="border-b border-line">
              <SheetTitle className="font-sans text-base tracking-tight">
                {joint.label}
              </SheetTitle>
              <SheetDescription className="micro-label">
                {joint.id} · {joint.onCan ? 'on can' : 'description only'}
              </SheetDescription>
            </SheetHeader>

            <div className="flex-1 overflow-y-auto px-6 py-2">
              {joint.fields.map((field) => (
                <div
                  key={field.id}
                  className={cn(
                    'grid grid-cols-[1fr_auto] items-start gap-x-3 border-b border-line py-2.5',
                    field.warn && 'border-l-2 border-l-accent pl-2',
                  )}
                >
                  <div className="min-w-0">
                    <div className="text-sm text-foreground">{field.label}</div>
                    <div className="micro-label mt-0.5">
                      {field.source}
                      {field.warn ? <span className="text-accent"> · gap</span> : null}
                    </div>
                  </div>
                  <div className="text-right">
                    <div className="data-value text-sm text-foreground">{field.value}</div>
                    {field.incoming ? (
                      <button
                        type="button"
                        className="data-value mt-1 text-[11px] text-accent underline-offset-2 hover:underline"
                        onClick={() => onAcceptIncoming?.(field.id)}
                      >
                        incoming {field.incoming} · accept
                      </button>
                    ) : null}
                  </div>
                </div>
              ))}
            </div>

            <SheetFooter className="border-t border-line">
              <Button
                type="button"
                variant={incomingCount > 0 ? 'default' : 'secondary'}
                className="w-full"
                disabled={incomingCount === 0}
              >
                {incomingCount > 0
                  ? `Accept ${incomingCount} incoming → active`
                  : 'No incoming changes'}
              </Button>
            </SheetFooter>
          </>
        ) : null}
      </SheetContent>
    </Sheet>
  );
}
