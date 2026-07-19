import { Canvas } from '@react-three/fiber';

import { DustBackdrop } from '@/components/dashboard/layout/dust-backdrop';
import { sceneBackgroundClassName } from '@/components/dashboard/layout/constants';

type SceneBackgroundProps = {
  /** Freeze the dust loop during route transitions so React can commit. */
  paused?: boolean;
};

/** Fullscreen ambient host — stays mounted for the session via RootLayout. */
export function SceneBackground({ paused = false }: SceneBackgroundProps) {
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
        frameloop={paused ? 'never' : 'always'}
        gl={{ antialias: false, alpha: false, powerPreference: 'low-power' }}
      >
        <DustBackdrop />
      </Canvas>
    </div>
  );
}
