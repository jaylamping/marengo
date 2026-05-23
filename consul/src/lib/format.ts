export function formatRamUsage(usedGb: number, totalGb: number): string {
  const percent = Math.round((usedGb / totalGb) * 100);
  return `${usedGb} / ${totalGb} GB · ${percent}%`;
}

export function formatPercent(value: number): string {
  return `${value}%`;
}

export function formatTempC(value: number): string {
  return `${value.toFixed(1)}°C`;
}

export function formatLoad(value: number): string {
  return value.toFixed(2);
}
