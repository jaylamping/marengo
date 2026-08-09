import { GridTableIcon, ThreeDViewIcon, Upload01Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';

import {
  buildHardwareFacetSnapshots,
  buildHardwareRows,
  countCompletenessWarnings,
} from '@/components/dashboard/hardware/build-hardware-rows';
import { CommissioningAggregation } from '@/components/dashboard/hardware/commissioning-aggregation';
import {
  CompletenessSummaryBadge,
  StatusLegend,
} from '@/components/dashboard/hardware/completeness-chrome';
import { Hardware3dView } from '@/components/dashboard/hardware/hardware-3d-view';
import { HardwareSettingsSheet } from '@/components/dashboard/hardware/hardware-settings-sheet';
import { HardwareTable } from '@/components/dashboard/hardware/hardware-table';
import { ImportWizard } from '@/components/dashboard/hardware/import-wizard';
import { Button } from '@/components/ui/button';
import {
  ToggleGroup,
  ToggleGroupItem,
} from '@/components/ui/toggle-group';
import { useConfigSnapshot } from '@/hooks/use-config-snapshot';
import { robotWireFacetsLive } from '@/lib/commissioning';
import { fetchCompleteness } from '@/lib/hardware-api';
import { queryKeys } from '@/lib/query-keys';
import { useActuatorStore } from '@/state/actuatorStore';
import { useRobotStore } from '@/state/robotStore';

type ViewMode = 'table' | 'stage';

export function HardwareOverview() {
  const [view, setView] = useState<ViewMode>('table');
  const [selectedJoint, setSelectedJoint] = useState<string | null>(null);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [rangeOverrides, setRangeOverrides] = useState<Record<string, string>>({});

  const { data: snapshot } = useConfigSnapshot();
  const limitSnapshot = useActuatorStore((s) => s.limitSnapshot);
  const robotState = useRobotStore((s) => s.robotState);
  const wireLive = robotWireFacetsLive(robotState);

  const completenessQuery = useQuery({
    queryKey: queryKeys.hardwareCompleteness,
    queryFn: fetchCompleteness,
    staleTime: 30_000,
  });

  const warnings = completenessQuery.data ? completenessQuery.data.warnings : [];
  const completenessState = completenessQuery.isError
    ? 'error'
    : completenessQuery.data
      ? 'ok'
      : 'unknown';
  const rows = useMemo(
    () => buildHardwareRows(snapshot ?? null, warnings, limitSnapshot, robotState),
    [snapshot, warnings, limitSnapshot, robotState],
  );
  const facets = useMemo(
    () => buildHardwareFacetSnapshots(snapshot ?? null, robotState),
    [snapshot, robotState],
  );

  const rowsWithRange = useMemo(
    () =>
      rows.map((row) => ({
        ...row,
        liveRange: rangeOverrides[row.joint] ?? row.liveRange,
      })),
    [rows, rangeOverrides],
  );

  const selectedRow = useMemo(
    () => rowsWithRange.find((r) => r.joint === selectedJoint) ?? null,
    [rowsWithRange, selectedJoint],
  );

  const warnCount = countCompletenessWarnings(warnings);
  const onCanCount = rows.filter((r) => r.onCan && r.warningCount === 0).length;
  const gapCount = rows.filter((r) => r.warningCount > 0).length;
  const descriptionOnlyCount = rows.filter((r) => !r.onCan && r.warningCount === 0).length;

  const selectJoint = (joint: string | null) => {
    if (!joint) {
      return;
    }
    setSelectedJoint(joint);
    setSheetOpen(true);
  };

  return (
    <div
      className="flex min-h-0 flex-1 flex-col gap-4 p-4"
      data-testid="hardware-overview"
    >
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="font-sans text-lg tracking-tight text-foreground">Hardware</h1>
          <p className="micro-label">
            Master config + URDF · warn-only completeness
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <CompletenessSummaryBadge count={warnCount} state={completenessState} />
          <Button
            type="button"
            size="sm"
            variant="secondary"
            className="gap-1.5"
            data-testid="hardware-enable-ready-in-scope"
            disabled
            title={
              wireLive
                ? 'Enable all Ready in scope lands with Phase 5 targeted enable'
                : 'Enable gated until live wire facets (non-UNSPECIFIED homing_state)'
            }
          >
            Enable all Ready in scope
          </Button>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            className="gap-1.5"
            data-testid="hardware-import-btn"
            onClick={() => setImportOpen(true)}
          >
            <HugeiconsIcon icon={Upload01Icon} size={16} />
            Import
          </Button>
          <ToggleGroup
            multiple={false}
            value={[view]}
            onValueChange={(value) => {
              const next = value[0];
              if (next === 'table' || next === 'stage') {
                setView(next);
              }
            }}
            className="border border-line rounded-lg"
          >
            <ToggleGroupItem value="table" aria-label="Table view" className="gap-1 px-3">
              <HugeiconsIcon icon={GridTableIcon} size={14} />
              <span className="micro-label">Table</span>
            </ToggleGroupItem>
            <ToggleGroupItem value="stage" aria-label="3D view" className="gap-1 px-3">
              <HugeiconsIcon icon={ThreeDViewIcon} size={14} />
              <span className="micro-label">3D</span>
            </ToggleGroupItem>
          </ToggleGroup>
        </div>
      </div>

      <StatusLegend
        onCanCount={onCanCount}
        gapCount={gapCount}
        descriptionOnlyCount={descriptionOnlyCount}
      />

      <CommissioningAggregation facets={facets} />

      {view === 'table' ? (
        <HardwareTable
          rows={rowsWithRange}
          selectedJoint={selectedJoint}
          onSelect={selectJoint}
        />
      ) : (
        <Hardware3dView
          rows={rowsWithRange}
          selectedJoint={selectedJoint}
          onSelect={selectJoint}
        />
      )}

      <HardwareSettingsSheet
        row={selectedRow}
        open={sheetOpen}
        onOpenChange={setSheetOpen}
        onApplyRange={(range) => {
          if (selectedJoint) {
            setRangeOverrides((prev) => ({ ...prev, [selectedJoint]: range }));
          }
        }}
      />

      <ImportWizard
        open={importOpen}
        onOpenChange={setImportOpen}
        onActivated={() => {
          void completenessQuery.refetch();
        }}
      />
    </div>
  );
}
