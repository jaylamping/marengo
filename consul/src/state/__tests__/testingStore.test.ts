import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fromBinary, toBinary } from '@bufbuild/protobuf';

import { ControlMode, MitCommandBatchSchema } from '@/gen/marengo/v1/marengo_pb';
import { postEnableCommand, postTestingMitCommandBatch } from '@/lib/gateway-api';
import { useCompoundStore } from '@/state/compoundStore';
import { useRobotStore } from '@/state/robotStore';
import { useTestingStore } from '@/state/testingStore';

vi.mock('@/lib/gateway-api', () => ({
  postEnableCommand: vi.fn(),
  postTestingMitCommandBatch: vi.fn(),
}));

const JOINT = 'right_elbow_pitch';

beforeEach(() => {
  vi.mocked(postEnableCommand).mockReset();
  vi.mocked(postTestingMitCommandBatch).mockReset();
  useTestingStore.setState({ selectedJointNames: [JOINT], isRunning: true, dryRun: false });
  useCompoundStore.getState().setIsRunning(true);
  useRobotStore.setState({ robotState: null });
});

describe('testing store stop paths', () => {
  it('cancels every runner before a disable that fails (UI-01)', async () => {
    vi.mocked(postEnableCommand).mockRejectedValue(new Error('disable failed: 502'));
    const pending = useTestingStore.getState().disable();
    expect(useTestingStore.getState().isRunning).toBe(false);
    expect(useCompoundStore.getState().isRunning).toBe(false);
    await expect(pending).rejects.toThrow('502');
    expect(postEnableCommand).toHaveBeenCalledWith(false);
  });

  it('Stop holds the joint where it is measured (UI-05)', async () => {
    useRobotStore.setState({
      robotState: { joints: [{ name: JOINT, position: 0.42 }] } as never,
    });
    await useTestingStore.getState().stopTest();
    expect(useTestingStore.getState().isRunning).toBe(false);
    const batch = vi.mocked(postTestingMitCommandBatch).mock.calls[0]?.[0];
    expect(batch).toBeDefined();
    const sent = fromBinary(MitCommandBatchSchema, toBinary(MitCommandBatchSchema, batch!));
    expect(sent.mode).toBe(ControlMode.POSITION);
    expect(sent.joints.map((j) => [j.name, j.position])).toEqual([[JOINT, 0.42]]);
  });

  it('Stop without a measured pose disables instead of doing nothing (UI-05)', async () => {
    await useTestingStore.getState().stopTest();
    expect(postTestingMitCommandBatch).not.toHaveBeenCalled();
    expect(postEnableCommand).toHaveBeenCalledWith(false);
  });
});

describe('testing store start path', () => {
  it('a refused Start Hold leaves the hold not running and rejects (UI-13)', async () => {
    useTestingStore.setState({ isRunning: false });
    vi.mocked(postTestingMitCommandBatch).mockRejectedValue(
      new Error('joint not command-eligible: left_elbow'),
    );
    await expect(useTestingStore.getState().startTest()).rejects.toThrow('command-eligible');
    expect(useTestingStore.getState().isRunning).toBe(false);
  });
});
