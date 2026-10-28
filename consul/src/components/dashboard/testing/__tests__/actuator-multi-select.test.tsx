// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { create } from '@bufbuild/protobuf';
import { act, cleanup, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import { ActuatorMultiSelect } from '@/components/dashboard/testing/actuator-multi-select';
import { JointStateSchema, RobotStateSchema } from '@/gen/marengo/v1/marengo_pb';
import type { ConfigSnapshotDto } from '@/lib/config-api';
import { useRobotStore } from '@/state/robotStore';
import { useTestingStore } from '@/state/testingStore';

let snapshot: ConfigSnapshotDto | null = null;
vi.mock('@/hooks/use-config-snapshot', () => ({
  useConfigSnapshot: () => ({ data: snapshot }),
}));

function robotStateWith(names: string[]) {
  return create(RobotStateSchema, {
    timestampMs: 1n,
    joints: names.map((name) => create(JointStateSchema, { name })),
  });
}

beforeEach(() => {
  snapshot = null;
  useTestingStore.setState({ selectedJointNames: [] });
  useRobotStore.setState({ robotState: null });
});

afterEach(() => {
  cleanup();
  useRobotStore.setState({ robotState: null });
});

it('a Control-only credential (snapshot refused) still gets the live joints', () => {
  // fetchConfigSnapshot returns null when the gateway refuses /config/snapshot.
  useRobotStore.setState({
    robotState: robotStateWith(['right_shoulder_pitch', 'right_elbow_pitch']),
  });
  render(<ActuatorMultiSelect />);
  expect(screen.getByText('right_shoulder_pitch')).toBeInTheDocument();
  expect(screen.getByText('right_elbow_pitch')).toBeInTheDocument();
  expect(screen.getByText(/0 \/ 2 selected/)).toBeInTheDocument();
  expect(screen.getByTestId('joint-source-telemetry')).toBeInTheDocument();
});

it('a joint stays listed after it drops out of RobotState', () => {
  useRobotStore.setState({
    robotState: robotStateWith(['right_shoulder_pitch', 'right_elbow_pitch']),
  });
  render(<ActuatorMultiSelect />);
  act(() => useRobotStore.setState({ robotState: robotStateWith(['right_shoulder_pitch']) }));
  expect(screen.getByText('right_elbow_pitch')).toBeInTheDocument();
});

it('the config snapshot wins when it is available', () => {
  snapshot = {
    profile: 'arm',
    config_dir: '/opt/marengo/config',
    joints: ['right_elbow_pitch'],
    motors: [],
    control_limits: [],
  };
  useRobotStore.setState({
    robotState: robotStateWith(['right_shoulder_pitch', 'right_elbow_pitch']),
  });
  render(<ActuatorMultiSelect />);
  expect(screen.getByText('right_elbow_pitch')).toBeInTheDocument();
  expect(screen.queryByText('right_shoulder_pitch')).not.toBeInTheDocument();
  expect(screen.queryByTestId('joint-source-telemetry')).not.toBeInTheDocument();
});

it('with neither source it says why the list is empty', () => {
  render(<ActuatorMultiSelect />);
  expect(screen.getByText(/Joint list unavailable/)).toBeInTheDocument();
});
