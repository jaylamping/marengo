export function formatUptime(seconds: bigint | number): string {
  const total = Number(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return `${minutes}m`;
}

export function bytesToGb(bytes: bigint | number): number {
  return Number(bytes) / 1024 ** 3;
}
