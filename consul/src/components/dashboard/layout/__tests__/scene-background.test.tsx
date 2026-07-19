// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { sceneBackgroundClassName } from '@/components/dashboard/layout/constants';
import { SceneBackground } from '@/components/dashboard/layout/scene-background';

vi.mock('@react-three/fiber', () => ({
  Canvas: ({
    children,
    frameloop,
  }: {
    children: React.ReactNode;
    frameloop?: string;
  }) => (
    <div data-testid="r3f-canvas" data-frameloop={frameloop ?? 'always'}>
      {children}
    </div>
  ),
}));

vi.mock('@/components/dashboard/layout/dust-backdrop', () => ({
  DustBackdrop: () => <div data-testid="dust-backdrop" />,
}));

afterEach(() => {
  cleanup();
});

describe('SceneBackground', () => {
  it('mounts fullscreen z-0 canvas host with dust backdrop only', () => {
    render(<SceneBackground />);

    const host = screen.getByTestId('scene-background');
    expect(host.className).toBe(sceneBackgroundClassName);
    expect(screen.getByTestId('r3f-canvas')).toBeTruthy();
    expect(screen.getByTestId('dust-backdrop')).toBeTruthy();
    expect(screen.queryByTestId('urdf-scene')).toBeNull();
  });

  it('pauses the R3F frameloop when requested', () => {
    render(<SceneBackground paused />);
    expect(screen.getByTestId('r3f-canvas').getAttribute('data-frameloop')).toBe(
      'never',
    );
  });
});
