import type { CandumpFrameDto } from '@/lib/log-api';

type Props = {
  frames: CandumpFrameDto[];
  total: number;
  offset: number;
  pageSize: number;
  onPage: (offset: number) => void;
};

export function CandumpFrameTable({ frames, total, offset, pageSize, onPage }: Props) {
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex items-center justify-between text-sm text-muted-foreground">
        <span>
          {total} frames · showing {offset + 1}–{Math.min(offset + pageSize, total)}
        </span>
        <div className="flex gap-2">
          <button
            type="button"
            className="rounded border px-2 py-1 disabled:opacity-40"
            disabled={offset <= 0}
            onClick={() => onPage(Math.max(0, offset - pageSize))}
          >
            Prev
          </button>
          <button
            type="button"
            className="rounded border px-2 py-1 disabled:opacity-40"
            disabled={offset + pageSize >= total}
            onClick={() => onPage(offset + pageSize)}
          >
            Next
          </button>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-auto rounded-lg border">
        <table className="w-full text-xs">
          <thead className="sticky top-0 bg-card">
            <tr className="border-b text-left text-muted-foreground">
              <th className="p-2">Δt</th>
              <th className="p-2">if</th>
              <th className="p-2">id</th>
              <th className="p-2">data</th>
            </tr>
          </thead>
          <tbody>
            {frames.map((frame) => (
              <tr key={frame.line_no} className="border-b font-mono">
                <td className="p-2">{frame.delta_s.toFixed(6)}</td>
                <td className="p-2">{frame.interface}</td>
                <td className="p-2">{frame.can_id}</td>
                <td className="p-2">{frame.data}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
