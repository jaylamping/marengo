import { Canvas } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';

import { SHOULDER_PITCH_RIGHT_ONLY_URDF } from '@/assets/urdf/shoulder-pitch-right-only';
import { sceneBackgroundClassName } from '@/components/dashboard/layout/constants';
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
          gl={{ antialias: true }}
        >
          <ambientLight intensity={0.5} />
          <directionalLight position={[10, 10, 5]} intensity={1} />
          <UrdfScene />
          <OrbitControls makeDefault enabled={false} />
        </Canvas>
      </RobotModelProvider>
    </div>
  );
}
