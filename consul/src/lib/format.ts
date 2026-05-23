export function computeRamUsagePercent(usedGb: number, totalGb: number): number {
  if (totalGb <= 0) {
    return 0;
  }

  return Math.round((usedGb / totalGb) * 100);
}

export function formatRamUsage(usedGb: number, totalGb: number): string {
  const percent = computeRamUsagePercent(usedGb, totalGb);
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

export function formatVoltageV(value: number): string {
  return `${value.toFixed(1)} V`;
}

export function formatCurrentA(value: number): string {
  return `${value.toFixed(1)} A`;
}

export function formatPowerW(value: number): string {
  return `${Math.round(value)} W`;
}

export function formatEnergyWh(value: number): string {
  return `${value.toFixed(2)} Wh`;
}

export function formatBatteryPercent(value: number): string {
  return `${value}%`;
}

export function formatRuntimeMin(value: number): string {
  if (value < 60) {
    return `${value} min`;
  }

  const hours = Math.floor(value / 60);
  const minutes = value % 60;
  return minutes === 0 ? `${hours} h` : `${hours} h ${minutes} m`;
}
