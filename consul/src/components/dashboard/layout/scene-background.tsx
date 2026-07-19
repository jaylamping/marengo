import { Canvas } from '@react-three/fiber';

import { DustBackdrop } from '@/components/dashboard/layout/dust-backdrop';
import { sceneBackgroundClassName } from '@/components/dashboard/layout/constants';

/** Fullscreen ambient host — stays mounted for the session via RootLayout. */
export function SceneBackground() {
  return (
    <div
      data-testid="scene-background"
      className={sceneBackgroundClassName}
      aria-hidden
    >
      <Canvas
        className="h-full w-full"
        camera={{ position: [0, 0, 4.5], fov: 50, near: 0.1, far: 40 }}
        dpr={1}
        frameloop="always"
        gl={{ antialias: false, alpha: false, powerPreference: 'low-power' }}
      >
        <DustBackdrop />
      </Canvas>
    </div>
  );
}
