import { create } from '@bufbuild/protobuf';
import { describe, expect, it, vi } from 'vitest';

import { livePiMetrics } from '@/components/dashboard/cards/pi-host-card';
import {
  HostMetricsSchema,
  MemoryMetricsSchema,
  PiPlatformMetricsSchema,
  ThermalMetricsSchema,
} from '@/gen/marengo/v1/marengo_pb';

vi.mock('@/lib/chappe-config', () => ({ isChappeLive: () => true }));

describe('livePiMetrics (L-marengo-host-metrics-02/-09)', () => {
  it('keeps unreadable memory, thermal and throttle state unknown, never zero/clear', () => {
    const metrics = livePiMetrics(
      create(HostMetricsSchema, {
        memory: create(MemoryMetricsSchema, { meminfoKnown: false }),
        thermal: create(ThermalMetricsSchema, { cpuKnown: false, cpuCelsius: 0 }),
        platform: {
          case: 'pi',
          value: create(PiPlatformMetricsSchema, { throttleKnown: false }),
        },
      }),
    );
    expect(metrics?.ramUsedGb).toBeNull();
    expect(metrics?.ramTotalGb).toBeNull();
    expect(metrics?.tempC).toBeNull();
    expect(metrics?.throttled).toBeNull();
    expect(metrics?.simulated).toBe(false);
  });

  it('shows known observations, including an honest 0 °C reading', () => {
    const metrics = livePiMetrics(
      create(HostMetricsSchema, {
        memory: create(MemoryMetricsSchema, {
          meminfoKnown: true,
          totalBytes: 8n * 1024n ** 3n,
          usedBytes: 2n * 1024n ** 3n,
        }),
        thermal: create(ThermalMetricsSchema, { cpuKnown: true, cpuCelsius: 0 }),
        platform: {
          case: 'pi',
          value: create(PiPlatformMetricsSchema, {
            throttleKnown: true,
            throttleEvents: 0x1,
          }),
        },
      }),
    );
    expect(metrics?.ramTotalGb).toBeGreaterThan(7);
    expect(metrics?.tempC).toBe(0);
    expect(metrics?.throttled).toBe(true);
  });

  it('marks the dev-host stub as simulated', () => {
    expect(livePiMetrics(create(HostMetricsSchema, { simulated: true }))?.simulated).toBe(true);
  });
});
