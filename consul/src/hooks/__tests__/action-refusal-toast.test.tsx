import { act, renderHook } from '@testing-library/react';
import { create, toBinary } from '@bufbuild/protobuf';
import { expect, it, vi } from 'vitest';
import { toast } from 'sonner';

import { ActionEventSchema, EnvelopeSchema } from '@/gen/marengo/v1/marengo_pb';
import type { ChappeTelemetryHandlers } from '@/lib/chappe-transport';
import { dispatchEnvelope } from '@/lib/chappe-transport';
import { getChappeSubscribeTopics } from '@/lib/chappe-config';
import { useChappeTelemetry } from '@/hooks/use-chappe-telemetry';
import { useRobotStore } from '@/state/robotStore';

let handlers: ChappeTelemetryHandlers;
vi.mock('sonner', () => ({ toast: { error: vi.fn() } }));
vi.mock('@/lib/chappe-client', () => ({
  connectChappeStream: vi.fn(async (next: ChappeTelemetryHandlers) => {
    handlers = next;
    return () => {};
  }),
}));
vi.mock('@/lib/chappe-config', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/lib/chappe-config')>()),
  isChappeLive: () => true,
}));
vi.mock('@/lib/log-buffer', () => ({
  enableChappeLiveLogs: () => {},
  appendLiveLog: () => {},
  shouldDecodeLogEvents: () => false,
}));

function actionEnvelope(accepted: boolean) {
  return toBinary(EnvelopeSchema, create(EnvelopeSchema, {
    sourceNode: 'marengo-pi',
    messageType: 'marengo.v1.ActionEvent',
    payload: toBinary(ActionEventSchema, create(ActionEventSchema, {
      action: 'motion_refused',
      joint: 'right_elbow_pitch',
      accepted,
      rejectReason: 'enable refused: motion is owned by stdin, not chappe',
    })),
  }));
}

it('subscribes to robot/audit/action (UI-07)', () => {
  expect(getChappeSubscribeTopics()).toContain('robot/audit/action');
});

it('a Pi refusal is toasted on any page; an accepted action is not (UI-07)', async () => {
  const mounted = renderHook(() => useChappeTelemetry());
  await act(async () => {});
  act(() => dispatchEnvelope(actionEnvelope(true), handlers));
  expect(toast.error).not.toHaveBeenCalled();
  act(() => dispatchEnvelope(actionEnvelope(false), handlers));
  expect(toast.error).toHaveBeenCalledWith(
    'Pi refused motion_refused (right_elbow_pitch): enable refused: motion is owned by stdin, not chappe',
  );
  mounted.unmount();
});

it('a Pi refusal is kept in the store for the Enable watch; an accepted action is not', async () => {
  useRobotStore.setState({ lastActionRefusal: null });
  const mounted = renderHook(() => useChappeTelemetry());
  await act(async () => {});
  act(() => dispatchEnvelope(actionEnvelope(true), handlers));
  expect(useRobotStore.getState().lastActionRefusal).toBeNull();
  act(() => dispatchEnvelope(actionEnvelope(false), handlers));
  expect(useRobotStore.getState().lastActionRefusal).toMatchObject({
    action: 'motion_refused',
    reason: 'enable refused: motion is owned by stdin, not chappe',
  });
  mounted.unmount();
  useRobotStore.setState({ lastActionRefusal: null });
});
