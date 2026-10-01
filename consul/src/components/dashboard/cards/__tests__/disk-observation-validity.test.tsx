import { act, cleanup, render, screen } from '@testing-library/react';
import { create, fromBinary, toBinary } from '@bufbuild/protobuf';
import { afterEach, expect, it, vi } from 'vitest';
import { HostMetricsSchema, DiskMetricsSchema, NetworkInterfaceMetricsSchema } from '@/gen/marengo/v1/marengo_pb';
import { PiHostCard } from '@/components/dashboard/cards/pi-host-card';
import { TooltipProvider } from '@/components/ui/tooltip';
import { useHostMetricsStore, diskWarning, canWarning } from '@/state/hostMetricsStore';
vi.mock('@/lib/chappe-config', () => ({ isChappeLive: () => true }));
afterEach(() => cleanup());
const metric = (capacityKnown: boolean, mountStatusKnown: boolean, readOnly = false) => fromBinary(HostMetricsSchema,
  toBinary(HostMetricsSchema, create(HostMetricsSchema, { disks: [create(DiskMetricsSchema, {
    mountPoint: '/', totalBytes: 2n * 1024n ** 3n, usedBytes: 1024n ** 3n,
    capacityKnown, mountStatusKnown, readOnly,
  })] })));

it('published unknown mount/capacity never yields a healthy disk diagnostic', () => {
  expect(diskWarning(metric(true, true))).toBe(false);
  expect(diskWarning(metric(true, true, true))).toBe(true);
  expect(diskWarning(metric(false, true))).toBe(true);
  expect(diskWarning(metric(true, false))).toBe(true);
  expect(diskWarning(create(HostMetricsSchema))).toBe(true);
});
it('card displays unknown capacity without a bar and restores only valid observations', () => {
  useHostMetricsStore.getState().setPiMetrics(metric(false, false));
  render(<TooltipProvider><PiHostCard /></TooltipProvider>);
  const disk = () => screen.getByText('Disk').parentElement!.parentElement!;
  expect(disk().textContent).toBe('Disk—');
  expect(disk().querySelector('[role="progressbar"]')).toBeNull();
  act(() => useHostMetricsStore.getState().setPiMetrics(metric(true, true)));
  expect(disk().textContent).not.toBe('Disk—');
  expect(disk().querySelector('[role="progressbar"]')?.getAttribute('aria-valuenow')).toBe('50');
  act(() => useHostMetricsStore.getState().setPiMetrics(metric(false, true)));
  expect(disk().textContent).toBe('Disk—');
  expect(disk().querySelector('[role="progressbar"]')).toBeNull();
});
it('published CAN unknown and failure states remain diagnostic warnings', () => {
  for (const [canState, expected] of [['ERROR-ACTIVE', false], ['UNKNOWN', true], ['', true], ['BUS-OFF', true]] as const) {
    const wire = toBinary(HostMetricsSchema, create(HostMetricsSchema, {
      network: [create(NetworkInterfaceMetricsSchema, { name: 'can0', canState })],
    }));
    expect(canWarning(fromBinary(HostMetricsSchema, wire))).toBe(expected);
  }
});
