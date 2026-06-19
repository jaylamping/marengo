// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { sceneBackgroundClassName } from '@/components/dashboard/layout/constants';
import { SceneBackground } from '@/components/dashboard/layout/scene-background';

vi.mock('@/assets/urdf/shoulder-pitch-right-only', () => ({
  SHOULDER_PITCH_RIGHT_ONLY_URDF: '<robot name="test" />',
}));

vi.mock('@react-three/fiber', () => ({
  Canvas: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="r3f-canvas">{children}</div>
  ),
}));

vi.mock('@react-three/drei', () => ({
  OrbitControls: () => <div data-testid="orbit-controls" />,
}));

vi.mock('@/urdf/RobotModelContext', () => ({
  RobotModelProvider: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="robot-model-provider">{children}</div>
  ),
}));

vi.mock('@/components/dashboard/urdf-preview/urdf-scene', () => ({
  UrdfScene: () => <div data-testid="urdf-scene" />,
}));

afterEach(() => {
  cleanup();
});

describe('SceneBackground', () => {
  it('mounts fullscreen z-0 canvas host with pointer-events disabled', () => {
    render(<SceneBackground />);

    const host = screen.getByTestId('scene-background');
    expect(host.className).toBe(sceneBackgroundClassName);
    expect(screen.getByTestId('r3f-canvas')).toBeTruthy();
    expect(screen.getByTestId('urdf-scene')).toBeTruthy();
  });

  it('wraps the scene in RobotModelProvider for shared URDF context', () => {
    render(<SceneBackground />);

    const provider = screen.getByTestId('robot-model-provider');
    const canvas = screen.getByTestId('r3f-canvas');
    expect(provider.contains(canvas)).toBe(true);
  });
});
