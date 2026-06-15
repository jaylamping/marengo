export function formatSigFig(value: number, sigFigs = 2): string {
  if (!Number.isFinite(value)) {
    return '—';
  }
  if (value === 0) {
    return '0';
  }

  const abs = Math.abs(value);
  const magnitude = Math.floor(Math.log10(abs));
  const decimals = Math.max(0, sigFigs - magnitude - 1);
  const factor = 10 ** decimals;
  const rounded = Math.round(value * factor) / factor;

  if (decimals === 0) {
    return String(Math.round(rounded));
  }

  return rounded.toFixed(decimals);
}

export function computeUsagePercent(used: number, total: number): number {
  if (total <= 0) {
    return 0;
  }

  return Math.round((used / total) * 100);
}

export function computeRamUsagePercent(usedGb: number, totalGb: number): number {
  return computeUsagePercent(usedGb, totalGb);
}

export function formatRamUsage(usedGb: number, totalGb: number): string {
  const percent = computeUsagePercent(usedGb, totalGb);
  return `${formatSigFig(usedGb)} / ${formatSigFig(totalGb)} GB · ${percent}%`;
}

export function formatPercent(value: number): string {
  return `${formatSigFig(value)}%`;
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
