import type { HostMetrics } from '@/gen/marengo/v1/marengo_pb';

export type HostDebugLine = {
  label: string;
  value: string;
};

function formatTimestampMs(timestampMs: bigint | number): string {
  return new Date(Number(timestampMs)).toISOString();
}

export function parseDeployRev(raw: string): { rev: string; deployedAt?: string } {
  const cleaned = raw.trim().replace(/\\n/g, '').replace(/\s+/g, ' ').trim();
  const [rev, ...rest] = cleaned.split(' ');
  if (rev && /^[0-9a-f]{7,40}$/i.test(rev)) {
    return {
      rev,
      deployedAt: rest.length > 0 ? rest.join(' ') : undefined,
    };
  }
  return { rev: cleaned };
}

export function hostDebugLinesFromMetrics(
  metrics: HostMetrics,
  options?: { subsystem?: string },
): HostDebugLine[] {
  const lines: HostDebugLine[] = [];

  if (metrics.hostname) {
    lines.push({ label: 'Host', value: metrics.hostname });
  }
  if (options?.subsystem) {
    lines.push({ label: 'Subsystem', value: options.subsystem });
  }
  const build = metrics.build;

  if (build?.deployRev) {
    const { rev, deployedAt } = parseDeployRev(build.deployRev);
    lines.push({ label: 'Deploy', value: rev });
    if (deployedAt) {
      lines.push({ label: 'Deployed', value: deployedAt });
    }
  }
  if (build?.gitSha) {
    lines.push({ label: 'Git', value: build.gitSha });
  }
  if (build?.semver) {
    lines.push({ label: 'Version', value: build.semver });
  }
  if (metrics.timestampMs) {
    lines.push({
      label: 'Telemetry',
      value: formatTimestampMs(metrics.timestampMs),
    });
  }
  if (metrics.kernelVersion) {
    lines.push({ label: 'Kernel', value: metrics.kernelVersion });
  }
  if (metrics.osPrettyName) {
    lines.push({ label: 'OS', value: metrics.osPrettyName });
  }

  return lines;
}
