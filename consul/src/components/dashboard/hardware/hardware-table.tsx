import { cn } from '@/lib/utils';

import type { HardwareJointRow } from '@/components/dashboard/hardware/build-hardware-rows';
import {
  JointStatusDot,
} from '@/components/dashboard/hardware/completeness-chrome';

type HardwareTableProps = {
  rows: HardwareJointRow[];
  selectedJoint: string | null;
  onSelect: (joint: string) => void;
};

export function HardwareTable({ rows, selectedJoint, onSelect }: HardwareTableProps) {
  return (
    <div className="min-h-0 flex-1 overflow-auto rounded-sm border border-line bg-surface-1">
      <table className="w-full border-collapse text-left">
        <thead className="sticky top-0 z-[1] bg-surface-2">
          <tr className="border-b border-line">
            <th className="micro-label px-3 py-2 font-medium">Status</th>
            <th className="micro-label px-3 py-2 font-medium">Joint</th>
            <th className="micro-label px-3 py-2 font-medium">CAN</th>
            <th className="micro-label px-3 py-2 font-medium">Range (live)</th>
            <th className="micro-label px-3 py-2 font-medium">Gaps</th>
            <th className="micro-label px-3 py-2 font-medium">Motor</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => {
            const selected = selectedJoint === row.joint;
            return (
              <tr
                key={row.joint}
                className={cn(
                  'cursor-pointer border-b border-line/70 transition-colors',
                  selected ? 'bg-surface-3' : 'hover:bg-surface-2',
                )}
                data-testid={`hardware-row-${row.joint}`}
                onClick={() => onSelect(row.joint)}
              >
                <td className="px-3 py-2.5">
                  <JointStatusDot
                    onCan={row.onCan}
                    hasWarnings={row.warningCount > 0}
                  />
                </td>
                <td className="px-3 py-2.5">
                  <div className="text-sm text-foreground">{row.joint}</div>
                </td>
                <td className="data-value px-3 py-2.5 text-sm">
                  {row.onCan
                    ? `${row.canInterface ?? 'can'} · id ${row.canId}`
                    : '—'}
                </td>
                <td className="data-value px-3 py-2.5 text-sm tabular-nums">
                  {row.liveRange}
                </td>
                <td className="px-3 py-2.5">
                  {row.warningCount > 0 ? (
                    <span className="data-value text-sm text-accent">
                      {row.warningCount}
                    </span>
                  ) : (
                    <span className="micro-label">0</span>
                  )}
                </td>
                <td className="micro-label px-3 py-2.5">
                  {row.motorType ?? '—'}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
