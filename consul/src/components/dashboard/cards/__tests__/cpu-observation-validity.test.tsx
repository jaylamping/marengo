import { act, cleanup, render, screen } from '@testing-library/react';
import { create, fromBinary, toBinary } from '@bufbuild/protobuf';
import { afterEach, expect, it, vi } from 'vitest';
import { HostMetricsSchema, CpuMetricsSchema } from '@/gen/marengo/v1/marengo_pb';
import { TooltipProvider } from '@/components/ui/tooltip';
import { PiHostCard } from '@/components/dashboard/cards/pi-host-card';
import { useHostMetricsStore } from '@/state/hostMetricsStore';

vi.mock('@/lib/chappe-config', () => ({ isChappeLive: () => true }));
vi.mock('@/components/dashboard/metrics/animated-number', () => ({
  AnimatedNumber: ({ value, format }: { value: number; format: (n: number) => string }) => <span>{format(value)}</span>,
}));
afterEach(() => cleanup());

for (const [Card, setter] of [[PiHostCard, 'setPiMetrics']] as const) {
  it(`${setter} displays unknown CPU until valid counters and retires it on reset`, () => {
    const publish = (sampleValid: boolean, usagePercent: number) => {
      const wire = toBinary(HostMetricsSchema, create(HostMetricsSchema, {
        cpu: create(CpuMetricsSchema, { sampleValid, usagePercent }),
      }));
      useHostMetricsStore.getState()[setter](fromBinary(HostMetricsSchema, wire));
    };
    publish(false, 0);
    render(<TooltipProvider><Card /></TooltipProvider>);
    const cpu = () => screen.getByText('CPU').parentElement!.parentElement!;
    expect(cpu().textContent).toBe('CPU—');
    expect(cpu().querySelector('[role="progressbar"]')).toBeNull();
    act(() => publish(true, 20));
    expect(cpu().textContent).toBe('CPU20%');
    expect(cpu().querySelector('[role="progressbar"]')?.getAttribute('aria-valuenow')).toBe('20');
    act(() => publish(false, 20));
    expect(cpu().textContent).toBe('CPU—');
    expect(cpu().querySelector('[role="progressbar"]')).toBeNull();
  });
}
