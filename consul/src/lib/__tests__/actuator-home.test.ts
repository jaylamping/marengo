import { describe, expect, it } from 'vitest';

import { actuatorHomeGate } from '@/lib/actuator-home';

describe('actuatorHomeGate', () => {
  const base = {
    interactive: true,
    connected: true,
    live: true,
    jointName: 'right_upper_arm_yaw',
    operationalMode: 'ACTIVE' as string | null,
    zeroed: true,
  };

  it('allows Home when zeroed and ACTIVE', () => {
    expect(actuatorHomeGate(base).ok).toBe(true);
  });

  it('blocks when not zeroed and not READY/ACTIVE', () => {
    const gate = actuatorHomeGate({
      ...base,
      zeroed: false,
      operationalMode: 'DISABLED',
    });
    expect(gate.ok).toBe(false);
    expect(gate.reason).toMatch(/Unlock zero|Set Zero|Verified/i);
  });

  it('treats READY as zeroed but still requires ACTIVE to move', () => {
    const gate = actuatorHomeGate({
      ...base,
      zeroed: false,
      operationalMode: 'READY',
    });
    expect(gate.ok).toBe(false);
    expect(gate.reason).toMatch(/Enable/i);
  });

  it('blocks when zeroed but not ACTIVE', () => {
    const gate = actuatorHomeGate({ ...base, operationalMode: 'READY' });
    expect(gate.ok).toBe(false);
    expect(gate.reason).toMatch(/Enable/i);
  });

  it('blocks when offline', () => {
    const gate = actuatorHomeGate({
      ...base,
      interactive: false,
      live: false,
    });
    expect(gate.ok).toBe(false);
  });
});
