import { describe, expect, it, vi, afterEach } from 'vitest';

import {
  demoBadge,
  isWireframeMode,
  jointChartCopy,
  resolveJointChartFeed,
} from '@/lib/telemetry-source';

vi.mock('@/lib/chappe-config', () => ({
  isChappeLive: vi.fn(() => false),
}));

import { isChappeLive } from '@/lib/chappe-config';

const isChappeLiveMock = vi.mocked(isChappeLive);

afterEach(() => {
  isChappeLiveMock.mockReset();
  isChappeLiveMock.mockReturnValue(false);
});

describe('telemetry-source', () => {
  it('treats non-live Chappe as wireframe', () => {
    expect(isWireframeMode()).toBe(true);
    isChappeLiveMock.mockReturnValue(true);
    expect(isWireframeMode()).toBe(false);
  });

  it('demo badges stay muted', () => {
    expect(demoBadge()).toEqual({ label: 'demo', tone: 'muted' });
  });

  it('resolves joint chart feed kinds', () => {
    expect(resolveJointChartFeed({ live: false, connected: false, pointCount: 0 })).toBe(
      'demo'
    );
    expect(resolveJointChartFeed({ live: true, connected: false, pointCount: 0 })).toBe(
      'waiting'
    );
    expect(resolveJointChartFeed({ live: true, connected: true, pointCount: 0 })).toBe(
      'waiting'
    );
    expect(resolveJointChartFeed({ live: true, connected: true, pointCount: 3 })).toBe(
      'live'
    );
  });

  it('never titles demo or waiting as live', () => {
    expect(jointChartCopy('right_shoulder_roll', 'demo').title).toContain('(demo)');
    expect(jointChartCopy('right_shoulder_roll', 'waiting').title).toContain('(waiting)');
    expect(jointChartCopy('right_shoulder_roll', 'live').title).toContain('(live)');
    expect(jointChartCopy('right_shoulder_roll', 'demo').title).not.toContain('(live)');
  });
});
