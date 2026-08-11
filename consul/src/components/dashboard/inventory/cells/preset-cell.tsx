/**
 * @deprecated Telemetry no longer shows a Preset column. Kept as a read-only
 * stub so leftover imports fail closed (display only, no assign).
 */
type PresetCellProps = {
  itemId: number;
  preset: string;
  jointName: string;
};

export function PresetCell({ preset, jointName }: PresetCellProps) {
  const isUnassigned = preset === 'unassigned';
  return (
    <span
      className="font-mono text-xs text-muted-foreground"
      data-testid="preset-cell-readonly"
      title={`${jointName} catalog tag (read-only)`}
    >
      {isUnassigned ? '—' : preset}
    </span>
  );
}
