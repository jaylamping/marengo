import { describe, expect, it, vi } from 'vitest';

import { persistJointLimits } from '@/lib/persist-joint-limits';

describe('persistJointLimits', () => {
  it('patches motors.yaml bench bounds from a proposed Range string', async () => {
    const patchConfig = vi.fn().mockResolvedValue({
      ok: true,
      message: 'Updated right_shoulder_pitch',
      restart_required: true,
    });

    const result = await persistJointLimits(
      'right_shoulder_pitch',
      '−0.50–1.20',
      { patchConfig },
    );

    expect(result).toEqual({
      ok: true,
      lower: -0.5,
      upper: 1.2,
      restartRequired: true,
      message: 'Updated right_shoulder_pitch',
    });
    expect(patchConfig).toHaveBeenCalledWith({
      joint: 'right_shoulder_pitch',
      position_lower_rad: -0.5,
      position_upper_rad: 1.2,
    });
  });

  it('rejects unparseable ranges without calling the gateway', async () => {
    const patchConfig = vi.fn();
    const result = await persistJointLimits('right_shoulder_roll', '—', {
      patchConfig,
    });
    expect(result.ok).toBe(false);
    expect(patchConfig).not.toHaveBeenCalled();
  });

  it('surfaces a gateway/auth failure when patchConfig returns null', async () => {
    const patchConfig = vi.fn().mockResolvedValue(null);
    const result = await persistJointLimits('right_shoulder_roll', '±1.57', {
      patchConfig,
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.message).toMatch(/gateway|auth|token|Chappe/i);
    }
  });
});
