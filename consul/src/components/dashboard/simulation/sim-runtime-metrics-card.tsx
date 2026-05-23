import { DashboardCardShell } from '@/components/dashboard/cards/dashboard-card-shell';
import { MetricGrid } from '@/components/dashboard/metrics/metric-grid';
import { MetricItem } from '@/components/dashboard/metrics/metric-item';
import type { SimRuntimeMetrics } from '@/data/simulation';
import { formatPercent, formatRamUsage } from '@/lib/format';

type SimRuntimeMetricsCardProps = {
  metrics: SimRuntimeMetrics;
};

export function SimRuntimeMetricsCard({ metrics }: SimRuntimeMetricsCardProps) {
  return (
    <DashboardCardShell
      description="Runtime"
      title="Physics + render"
      content={
        <MetricGrid>
          <MetricItem label="Sim time" value={`${metrics.simTimeS.toFixed(1)} s`} />
          <MetricItem label="RTF" value={metrics.realTimeFactor.toFixed(2)} />
          <MetricItem label="Physics dt" value={`${metrics.physicsDtMs.toFixed(2)} ms`} />
          <MetricItem label="Render FPS" value={String(metrics.renderFps)} />
          <MetricItem label="GPU" value={formatPercent(metrics.gpuUtilPercent)} />
          <MetricItem
            label="VRAM"
            value={formatRamUsage(metrics.gpuMemUsedGb, metrics.gpuMemTotalGb)}
            valueClassName="text-sm"
          />
        </MetricGrid>
      }
      footerPrimary="Metrics populate when sim is running"
      footerSecondary="Target RTF ≥ 0.8 for policy eval"
    />
  );
}
