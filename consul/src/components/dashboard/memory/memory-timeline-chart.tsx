import {
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';

import type { Mem0Memory } from '@/lib/mem0-config';

const COLORS: Record<string, string> = {
  sdd: '#6366f1',
  feasibility: '#f59e0b',
  research: '#10b981',
  expert: '#ec4899',
  maintenance: '#64748b',
  other: '#94a3b8',
};

function dayKey(iso?: string): string {
  if (!iso) {
    return 'unknown';
  }
  return iso.slice(0, 10);
}

export function buildTimelineData(memories: Mem0Memory[]) {
  const byDay = new Map<string, Record<string, number>>();

  for (const memory of memories) {
    const day = dayKey(memory.created_at ?? memory.updated_at);
    const bucket = byDay.get(day) ?? {};
    const ns = memory.namespace === 'all' ? 'other' : memory.namespace;
    bucket[ns] = (bucket[ns] ?? 0) + 1;
    byDay.set(day, bucket);
  }

  return [...byDay.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .slice(-14)
    .map(([date, counts]) => ({ date, ...counts }));
}

type MemoryTimelineChartProps = {
  memories: Mem0Memory[];
};

export function MemoryTimelineChart({ memories }: MemoryTimelineChartProps) {
  const data = buildTimelineData(memories);
  if (data.length === 0) {
    return (
      <div className="rounded-lg border border-dashed p-6 text-sm text-muted-foreground">
        No memories yet — SDD and research automations will populate this chart.
      </div>
    );
  }

  return (
    <div className="h-56 w-full rounded-lg border bg-card p-3">
      <ResponsiveContainer width="100%" height="100%">
        <BarChart data={data}>
          <CartesianGrid strokeDasharray="3 3" vertical={false} />
          <XAxis dataKey="date" tick={{ fontSize: 11 }} />
          <YAxis allowDecimals={false} tick={{ fontSize: 11 }} />
          <Tooltip />
          <Legend />
          {Object.keys(COLORS).map((key) => (
            <Bar key={key} dataKey={key} stackId="ns" fill={COLORS[key]} />
          ))}
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
