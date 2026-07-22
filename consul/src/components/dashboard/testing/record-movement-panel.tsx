import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Checkbox } from '@/components/ui/checkbox';
import { Slider } from '@/components/ui/slider';
import { useRecordMovementController } from '@/hooks/use-record-movement-controller';
import { canApplyLandmarks } from '@/lib/teach-record';
import { useTeachStore } from '@/state/teachStore';

interface RecordMovementPanelProps {
  presetId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function RecordMovementPanel({
  presetId,
  open,
  onOpenChange,
}: RecordMovementPanelProps) {
  const controller = useRecordMovementController(presetId);
  const setGravityArmed = useTeachStore((state) => state.setGravityArmed);
  const setLandmarkIncluded = useTeachStore(
    (state) => state.setLandmarkIncluded,
  );
  const setCadenceScale = useTeachStore((state) => state.setCadenceScale);
  const setSettleDwellSec = useTeachStore((state) => state.setSettleDwellSec);
  const markCalibrationChanged = useTeachStore(
    (state) => state.markCalibrationChanged,
  );
  const acknowledgeCalibration = useTeachStore(
    (state) => state.acknowledgeCalibration,
  );

  if (!controller.base || !open) return null;

  return (
    <Card variant="panel" className={dashboardPanelCardClassName}>
      <CardHeader className="flex flex-row items-start justify-between gap-4">
        <div>
          <CardTitle className="text-lg">Record Movement</CardTitle>
          <CardDescription>
            {controller.base.teach.appliedDescription}
          </CardDescription>
        </div>
        <Button size="sm" variant="ghost" onClick={() => onOpenChange(false)}>
          Close
        </Button>
      </CardHeader>
      <CardContent className="space-y-6">
        <div className="rounded-md border border-border/60 bg-muted/20 p-3 text-xs text-muted-foreground space-y-2">
          <p>
            Record only in GravityComp. Testing commands post Position or
            Impedance and end GravityComp.
          </p>
          <div className="flex items-center gap-2">
            <Checkbox
              id={`gravity-armed-${presetId}`}
              checked={controller.gravityArmed}
              onCheckedChange={(checked) => setGravityArmed(!!checked)}
              disabled={controller.recording}
            />
            <label
              htmlFor={`gravity-armed-${presetId}`}
              className="cursor-pointer font-medium text-foreground"
            >
              Gravity-comp armed and ACTIVE
            </label>
          </div>
          <div className="flex flex-wrap gap-2">
            <Badge variant="secondary">
              mode: {controller.operationalMode ?? 'unknown'}
            </Badge>
            <Badge variant="secondary">
              samples: {controller.hasDraft ? controller.samples.length : 0}
            </Badge>
            {controller.overlay ? <Badge>taught</Badge> : null}
            {controller.needsCalAck ? (
              <Badge variant="destructive">calibration ack needed</Badge>
            ) : null}
          </div>
        </div>
        {controller.needsCalAck ? (
          <div className="rounded-md border border-amber-500/40 bg-amber-500/10 p-3 text-xs space-y-2">
            <p>Calibration changed after this overlay was applied.</p>
            <div className="flex flex-wrap gap-2">
              <Button
                size="sm"
                onClick={() => acknowledgeCalibration(presetId)}
              >
                Acknowledge and keep
              </Button>
              <Button size="sm" variant="outline" onClick={controller.reset}>
                Reset overlay
              </Button>
            </div>
          </div>
        ) : null}
        <div className="flex flex-wrap gap-2">
          {controller.isRecordingThisPreset ? (
            <Button variant="destructive" onClick={controller.stopRecord}>
              Stop Record
            </Button>
          ) : (
            <Button
              onClick={controller.startRecord}
              disabled={!controller.canRecord || controller.recording}
            >
              Record Movement
            </Button>
          )}
          <Button
            variant="outline"
            onClick={controller.apply}
            disabled={
              controller.recording ||
              !controller.hasDraft ||
              !canApplyLandmarks(controller.landmarks)
            }
          >
            Apply to {controller.base.name}
          </Button>
          <Button
            variant="outline"
            onClick={controller.download}
            disabled={!controller.hasDraft && !controller.overlay}
          >
            Download
          </Button>
          <Button
            variant="secondary"
            onClick={markCalibrationChanged}
            disabled={!controller.overlay || controller.recording}
            title="Use after set-zero of teach joints"
          >
            I set-zero&apos;d
          </Button>
          <Button variant="ghost" onClick={controller.reset}>
            Reset
          </Button>
        </div>
        {controller.compoundRunning ? (
          <p className="text-xs text-muted-foreground">
            Stop the compound test before recording.
          </p>
        ) : null}
        {controller.returnHomePending ? (
          <p className="text-xs text-muted-foreground">
            Return-home is in progress.
          </p>
        ) : null}
        {controller.lastError ? (
          <p className="text-sm text-destructive" role="alert">
            {controller.lastError}
          </p>
        ) : null}
        <section className="space-y-3">
          <h4 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
            Cadence and dwell
          </h4>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="space-y-2">
              <div className="flex justify-between text-xs">
                <span>Cadence scale</span>
                <span>{controller.cadenceScale.toFixed(2)}×</span>
              </div>
              <Slider
                value={[controller.cadenceScale]}
                min={0.25}
                max={2}
                step={0.25}
                onValueChange={(value) => setCadenceScale(value[0])}
                disabled={controller.recording}
              />
            </div>
            <div className="space-y-2">
              <div className="flex justify-between text-xs">
                <span>Settle dwell</span>
                <span>{controller.settleDwellSec.toFixed(2)} s</span>
              </div>
              <Slider
                value={[controller.settleDwellSec]}
                min={0}
                max={1}
                step={0.05}
                onValueChange={(value) => setSettleDwellSec(value[0])}
                disabled={controller.recording}
              />
            </div>
          </div>
        </section>
        <section className="space-y-3">
          <h4 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
            Landmarks
          </h4>
          {!controller.hasDraft || controller.landmarks.length === 0 ? (
            <p className="text-xs text-muted-foreground italic">
              Stop recording to extract full-q landmarks.
            </p>
          ) : (
            <ul className="space-y-2">
              {controller.landmarks.map((landmark) => (
                <li
                  key={landmark.id}
                  className="flex items-start gap-3 rounded border border-border/50 bg-muted/20 p-2 text-xs"
                >
                  <Checkbox
                    checked={landmark.included}
                    onCheckedChange={(checked) =>
                      setLandmarkIncluded(landmark.id, !!checked)
                    }
                    id={landmark.id}
                  />
                  <label
                    htmlFor={landmark.id}
                    className="flex-1 cursor-pointer"
                  >
                    {landmark.label} at {landmark.tSec.toFixed(2)}s
                  </label>
                </li>
              ))}
            </ul>
          )}
        </section>
      </CardContent>
    </Card>
  );
}
