// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';

import { ActuatorsOverview } from '@/components/dashboard/actuators/actuators-overview';
import { JointCard } from '@/components/dashboard/actuators/joint-card';
import { WIRED_BENCH_JOINTS } from '@/data/actuator-joints';
import type { InventoryItem } from '@/data/robot-inventory';

vi.mock('@/hooks/use-live-inventory', () => ({
  useLiveInventory: (base: InventoryItem[]) => base,
}));

vi.mock('@/hooks/use-enriched-inventory', async () => {
  const { robotInventory } = await import('@/data/robot-inventory');
  return {
    useEnrichedInventory: () => ({
      data: robotInventory,
      status: 'success',
      fetchStatus: 'idle',
      isFetching: false,
      isPlaceholderData: false,
      dataUpdatedAt: Date.now(),
    }),
  };
});

vi.mock('@/state/robotStore', () => ({
  useRobotStore: (selector: (s: { connected: boolean }) => unknown) =>
    selector({ connected: false }),
}));

vi.mock('@/state/actuatorStore', async () => {
  const actual = await vi.importActual<typeof import('@/state/actuatorStore')>(
    '@/state/actuatorStore',
  );
  return {
    ...actual,
    useActuatorStore: (
      selector: (s: {
        bootstrap: { kind: 'ready'; clientId: string };
        limitSnapshot: null;
        lastError: null;
        nextCommandSeq: () => bigint;
        setLastError: (m: string | null) => void;
      }) => unknown,
    ) =>
      selector({
        bootstrap: { kind: 'ready', clientId: 'test-client' },
        limitSnapshot: null,
        lastError: null,
        nextCommandSeq: () => 1n,
        setLastError: () => undefined,
      }),
    liveJointLimits: vi.fn(() => null),
    jointLimitMax: vi.fn(() => null),
    selectClientId: vi.fn(() => 'test-client'),
  };
});

afterEach(() => {
  cleanup();
});

const wiredJoint: InventoryItem = {
  id: 25,
  name: 'right_shoulder_roll',
  group: 'right_arm',
  kind: 'actuator',
  status: 'Enabled',
  value: '0.12',
  limit: '±1.57',
  preset: 'bench_3dof',
  node: 'RS03 · can0 · id 1',
};

const unwiredJoint: InventoryItem = {
  id: 24,
  name: 'left_wrist',
  group: 'left_arm',
  kind: 'actuator',
  status: 'Offline',
  value: '—',
  limit: '±1.6',
  preset: 'unassigned',
  node: 'RS00 · can0 · id 18',
};

describe('JointCard read-only shell', () => {
  it('shows live position telemetry for a wired bench joint', () => {
    render(<JointCard joint={wiredJoint} wired />);

    expect(screen.getByText('right_shoulder_roll')).toBeInTheDocument();
    expect(screen.getByText('0.12 rad')).toBeInTheDocument();
    expect(screen.getByText(/bench wired/i)).toBeInTheDocument();
  });

  it('marks unwired inventory joints telemetry-only with explicit reason', () => {
    render(<JointCard joint={unwiredJoint} wired={false} />);

    expect(screen.getByText('left_wrist')).toBeInTheDocument();
    expect(screen.getByText(/not wired on bench/i)).toBeInTheDocument();
    expect(screen.getByText(/telemetry only/i)).toBeInTheDocument();
  });

  it('waits for live limits before exposing tuning sliders', () => {
    render(<JointCard joint={wiredJoint} wired />);

    expect(screen.getByTestId('tuning-panel-unavailable')).toBeInTheDocument();
    expect(screen.queryByRole('slider', { name: /runtime kp/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /jog/i })).not.toBeInTheDocument();
  });
});

describe('ActuatorsOverview', () => {
  it('renders joint cards for right 3-DOF wired joints plus inventory actuators', () => {
    render(<ActuatorsOverview />);

    const overview = screen.getByTestId('actuators-overview');
    expect(overview).toBeInTheDocument();

    for (const jointName of WIRED_BENCH_JOINTS) {
      expect(within(overview).getByText(jointName)).toBeInTheDocument();
    }

    expect(within(overview).getByText('left_wrist')).toBeInTheDocument();
    expect(
      within(overview).getByRole('heading', { name: /actuator harness/i }),
    ).toBeInTheDocument();
    expect(within(overview).getByText(/live limits required/i)).toBeInTheDocument();
  });

  it('renders every joint card without motion command controls', () => {
    render(<ActuatorsOverview />);

    const cards = screen.getAllByTestId('joint-card');
    expect(cards.length).toBeGreaterThanOrEqual(WIRED_BENCH_JOINTS.length);

    for (const card of cards) {
      expect(within(card).queryByRole('button', { name: /enable/i })).not.toBeInTheDocument();
    }
  });
});
