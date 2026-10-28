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
  useTestingStore.setState({
    selectedJointNames: [JOINT],
    isRunning: true,
    dryRun: false,
    runDryRun: null,
  });
  useCompoundStore.getState().setIsRunning(true);
  useRobotStore.setState({ robotState: null, operationalMode: 'ACTIVE' });
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

  it.each(['DISABLED', 'READY'] as const)(
    'Stop while %s sends nothing, so it cannot re-enable the drives',
    async (mode) => {
      // A safety or tick-error stop the operator did not order leaves the
      // run flagged; a hold batch now would make marengo-pi re-enable.
      useRobotStore.setState({
        operationalMode: mode,
        robotState: { joints: [{ name: JOINT, position: 0.42 }] } as never,
      });
      await useTestingStore.getState().stopTest();
      expect(useTestingStore.getState().isRunning).toBe(false);
      expect(postTestingMitCommandBatch).not.toHaveBeenCalled();
      expect(postEnableCommand).not.toHaveBeenCalled();
    },
  );

  it('a dry run stopped after the Dry Run toggle flips posts nothing', async () => {
    useTestingStore.setState({ isRunning: false, dryRun: true });
    useRobotStore.setState({
      robotState: { joints: [{ name: JOINT, position: 0.42 }] } as never,
    });
    await useTestingStore.getState().startTest();
    expect(postTestingMitCommandBatch).not.toHaveBeenCalled();
    useTestingStore.getState().toggleDryRun();
    expect(useTestingStore.getState().dryRun).toBe(false);
    await useTestingStore.getState().stopTest();
    expect(useTestingStore.getState().isRunning).toBe(false);
    expect(postTestingMitCommandBatch).not.toHaveBeenCalled();
    expect(postEnableCommand).not.toHaveBeenCalled();
  });

  it('Stop with an unknown mode disables rather than holding', async () => {
    useRobotStore.setState({
      operationalMode: null,
      robotState: { joints: [{ name: JOINT, position: 0.42 }] } as never,
    });
    await useTestingStore.getState().stopTest();
    expect(postTestingMitCommandBatch).not.toHaveBeenCalled();
    expect(postEnableCommand).toHaveBeenCalledWith(false);
  });

  it('Stop without a measured pose disables instead of doing nothing (UI-05)', async () => {
    await useTestingStore.getState().stopTest();
    expect(postTestingMitCommandBatch).not.toHaveBeenCalled();
    expect(postEnableCommand).toHaveBeenCalledWith(false);
  });
});
