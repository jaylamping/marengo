import { Canvas } from '@react-three/fiber';
import { ContactShadows, OrbitControls } from '@react-three/drei';

import { SHOULDER_PITCH_RIGHT_ONLY_URDF } from '@/assets/urdf/shoulder-pitch-right-only';
import { sceneBackgroundClassName } from '@/components/dashboard/layout/constants';
import { SpaceBackdrop } from '@/components/dashboard/layout/space-backdrop';
import { UrdfScene } from '@/components/dashboard/urdf-preview/urdf-scene';
import { RobotModelProvider } from '@/urdf/RobotModelContext';

export function SceneBackground() {
  return (
    <div
      data-testid="scene-background"
      className={sceneBackgroundClassName}
      aria-hidden
    >
      <RobotModelProvider urdfXml={SHOULDER_PITCH_RIGHT_ONLY_URDF}>
        <Canvas
          className="h-full w-full"
          camera={{ position: [0.5, 0.5, 0.5], fov: 50 }}
          dpr={[1, 1.5]}
          gl={{ antialias: true }}
        >
          <SpaceBackdrop />
          <ambientLight intensity={0.35} color="#9fb0cc" />
          <directionalLight position={[2, 3, 2]} intensity={1.1} color="#ffd9a8" />
          <directionalLight position={[-3, 1.5, -2]} intensity={0.5} color="#7aa2ff" />
          <UrdfScene />
          <ContactShadows
            position={[0, -0.001, 0]}
            opacity={0.5}
            scale={1.5}
            blur={2.2}
            far={0.5}
            resolution={256}
            color="#000000"
          />
          <OrbitControls makeDefault enabled={false} />
        </Canvas>
      </RobotModelProvider>
    </div>
  );
}
