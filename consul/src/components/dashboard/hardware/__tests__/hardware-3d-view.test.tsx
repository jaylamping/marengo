// @vitest-environment jsdom
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { Hardware3dView } from '@/components/dashboard/hardware/hardware-3d-view';
import type { HardwareJointRow } from '@/components/dashboard/hardware/build-hardware-rows';

const scenes = vi.hoisted(() => [] as { setSelected: ReturnType<typeof vi.fn> }[]);

vi.mock('@/components/dashboard/hardware/bench-arm-scene', () => ({
  BenchArmScene: vi.fn(function BenchArmScene(this: Record<string, unknown>) {
    this.setSelected = vi.fn();
    this.updateRows = vi.fn();
    this.dispose = vi.fn();
    scenes.push(this as never);
  }),
}));

function rowsFor(joints: string[], warningCount = 0): HardwareJointRow[] {
  return joints.map((joint) => ({ joint, warningCount, onCan: true }) as HardwareJointRow);
}

afterEach(() => {
  cleanup();
  scenes.length = 0;
});

describe('Hardware3dView (UI-16)', () => {
  it('keeps one scene while rows refresh with the same joints', () => {
    const props = { selectedJoint: 'b', onSelect: () => undefined };
    const { rerender } = render(<Hardware3dView rows={rowsFor(['a', 'b'])} {...props} />);
    for (let tick = 0; tick < 5; tick += 1) {
      rerender(<Hardware3dView rows={rowsFor(['a', 'b'], tick)} {...props} />);
    }
    expect(scenes).toHaveLength(1);
    expect(scenes[0]?.setSelected).toHaveBeenCalledWith('b');
  });

  it('rebuilds and re-applies the selection when the joint set changes', () => {
    const props = { selectedJoint: 'c', onSelect: () => undefined };
    const { rerender } = render(<Hardware3dView rows={rowsFor(['a'])} {...props} />);
    rerender(<Hardware3dView rows={rowsFor(['a', 'c'])} {...props} />);
    expect(scenes).toHaveLength(2);
    expect(scenes[1]?.setSelected).toHaveBeenCalledWith('c');
  });
});
