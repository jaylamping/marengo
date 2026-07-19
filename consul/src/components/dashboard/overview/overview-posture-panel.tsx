import { Suspense, lazy } from 'react';
import { Canvas } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';

import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { cn } from '@/lib/utils';
import { isChappeLive } from '@/lib/chappe-config';
import { useRobotStore } from '@/state/robotStore';

const UrdfScene = lazy(async () => {
  const module = await import('@/components/dashboard/urdf-preview/urdf-scene');
  return { default: module.UrdfScene };
});

type OverviewPosturePanelProps = {
  /** Pause the R3F loop while Overview is soft-cached off-route. */
  active?: boolean;
};

export function OverviewPosturePanel({ active = true }: OverviewPosturePanelProps) {
  const connected = useRobotStore((s) => s.connected);
  const live = isChappeLive();
  const feedLabel = !live ? 'Demo pose' : connected ? 'Live pose' : 'Waiting';

  return (
    <Card
      variant="panel"
      className={cn('@container/card flex h-full min-h-[20rem] flex-col', dashboardPanelCardClassName)}
      data-testid="overview-posture-panel"
    >
      <CardHeader className="shrink-0">
        <CardTitle>Posture</CardTitle>
        <CardDescription>{feedLabel} · URDF forward kinematics</CardDescription>
      </CardHeader>
      <CardContent className="relative min-h-0 flex-1 px-2 pb-4 sm:px-4">
        <div className="h-full min-h-[16rem] overflow-hidden rounded-[4px] border border-line bg-surface-0">
          <Canvas
            className="h-full w-full"
            camera={{ position: [0.55, 0.35, 0.85], fov: 42, near: 0.01, far: 20 }}
            dpr={1}
            frameloop={active ? 'always' : 'never'}
            gl={{ antialias: false, alpha: false, powerPreference: 'low-power' }}
          >
            <color attach="background" args={['#1a1d24']} />
            <ambientLight intensity={0.55} />
            <directionalLight position={[2.5, 3.5, 2]} intensity={1.1} />
            <Suspense fallback={null}>
              <UrdfScene />
            </Suspense>
            <OrbitControls
              enablePan={false}
              minDistance={0.35}
              maxDistance={2.2}
              target={[0, 0.15, 0]}
            />
          </Canvas>
        </div>
      </CardContent>
    </Card>
  );
}
