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

vi.mock('@/state/robotStore', () => ({
  useRobotStore: (selector: (s: { connected: boolean }) => unknown) =>
    selector({ connected: false }),
}));

afterEach(() => {
  cleanup();
});

const wiredJoint: InventoryItem = {
  id: 20,
  name: 'left_shoulder_roll',
  group: 'left_arm',
  kind: 'actuator',
  status: 'Enabled',
  value: '0.12',
  limit: '±1.57',
  preset: 'bench_default',
  node: 'RS03 · can0 · id 14',
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

    expect(screen.getByText('left_shoulder_roll')).toBeInTheDocument();
    expect(screen.getByText('0.12 rad')).toBeInTheDocument();
    expect(screen.getByText(/bench wired/i)).toBeInTheDocument();
  });

  it('marks unwired inventory joints telemetry-only with explicit reason', () => {
    render(<JointCard joint={unwiredJoint} wired={false} />);

    expect(screen.getByText('left_wrist')).toBeInTheDocument();
    expect(screen.getByText(/not wired on bench/i)).toBeInTheDocument();
    expect(screen.getByText(/telemetry only/i)).toBeInTheDocument();
  });

  it('does not expose motion or tuning command controls', () => {
    render(<JointCard joint={wiredJoint} wired />);

    expect(screen.queryByRole('button', { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /jog/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('slider')).not.toBeInTheDocument();
  });
});

describe('ActuatorsOverview', () => {
  it('renders joint cards for all four bench-wired joints plus inventory actuators', () => {
    render(<ActuatorsOverview />);

    const overview = screen.getByTestId('actuators-overview');
    expect(overview).toBeInTheDocument();

    for (const jointName of WIRED_BENCH_JOINTS) {
      expect(within(overview).getByText(jointName)).toBeInTheDocument();
    }

    expect(within(overview).getByText('left_wrist')).toBeInTheDocument();
    expect(within(overview).getByText(/read-only/i)).toBeInTheDocument();
  });

  it('renders every joint card without command controls', () => {
    render(<ActuatorsOverview />);

    const cards = screen.getAllByTestId('joint-card');
    expect(cards.length).toBeGreaterThanOrEqual(WIRED_BENCH_JOINTS.length);

    for (const card of cards) {
      expect(within(card).queryByRole('button', { name: /enable/i })).not.toBeInTheDocument();
    }
  });
});
