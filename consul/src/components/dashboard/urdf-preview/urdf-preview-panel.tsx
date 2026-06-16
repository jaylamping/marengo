import { useRef, useMemo } from 'react';
import { Canvas } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import * as THREE from 'three';

import { useRobotModel } from '@/urdf/RobotModelContext';
import { computeForwardKinematics } from '@/urdf/forward-kinematics';
import { useRobotStore } from '@/state/robotStore';

// A simple box to represent each link. Size is derived from a rough heuristic
// or could be passed via URDF <visual> in the future.
function LinkPrimitive({ name, isActive, matrix }: { name: string; isActive: boolean; matrix: THREE.Matrix4 }) {
  const meshRef = useRef<THREE.Mesh>(null);

  if (!meshRef.current) return null;

  // Apply the computed world matrix to the mesh
  meshRef.current.matrix = matrix;
  meshRef.current.matrixAutoUpdate = false;

  return (
    <mesh ref={meshRef} castShadow receiveShadow>
      <boxGeometry args={[0.1, 0.1, 0.2]} />
      <meshStandardMaterial
        color={isActive ? '#3b82f6' : '#94a3b8'}
        roughness={0.5}
        metalness={0.2}
      />
    </mesh>
  );
}

function UrdfScene() {
  const model = useRobotModel();
  const jointPositions = useRobotStore((s) => {
    const positions: Record<string, number> = {};
    for (const [jointName, points] of Object.entries(s.trackingPointsByJoint)) {
      if (points.length > 0) {
        // Use the last measured point
        positions[jointName] = points[points.length - 1].measured;
      } else {
        positions[jointName] = 0; // default to 0 if no data
      }
    }
    return positions;
  });

  const worldMatrices = useMemo(
    () => computeForwardKinematics(model, jointPositions),
    [model, jointPositions],
  );

  return (
    <group>
      <primitive object={new THREE.AxesHelper(0.5)} />
      {model.rootLinks.map((linkName) => (
        <LinkGroup key={linkName} linkName={linkName} model={model} worldMatrices={worldMatrices} visited={new Set()} />
      ))}
    </group>
  );
}

// Recursive component to build the kinematic chain visually
function LinkGroup({
  linkName,
  model,
  worldMatrices,
  visited,
}: {
  linkName: string;
  model: ReturnType<typeof useRobotModel>;
  worldMatrices: Map<string, THREE.Matrix4>;
  visited: Set<string>;
}) {
  if (visited.has(linkName)) return null;
  visited.add(linkName);

  // Find joints where this link is the parent
  const childJoints: string[] = [];
  for (const joint of model.joints.values()) {
    if (joint.parent === linkName) {
      childJoints.push(joint.child);
    }
  }

  const matrix = worldMatrices.get(linkName) || new THREE.Matrix4().identity();

  return (
    <>
      <LinkPrimitive name={linkName} isActive={linkName !== 'base_link'} matrix={matrix} />
      {childJoints.map((childLinkName) => (
        <LinkGroup
          key={childLinkName}
          linkName={childLinkName}
          model={model}
          worldMatrices={worldMatrices}
          visited={visited}
        />
      ))}
    </>
  );
}

export function UrdfPreviewPanel() {
  return (
    <div className="h-[400px] w-full rounded-lg border bg-background">
      <Canvas
        camera={{ position: [0.5, 0.5, 0.5], fov: 50 }}
        shadows
        gl={{ antialias: true }}
      >
        <ambientLight intensity={0.5} />
        <directionalLight position={[10, 10, 5]} intensity={1} castShadow />
        <primitive object={new THREE.AxesHelper(0.5)} />
        <UrdfScene />
        <OrbitControls makeDefault />
      </Canvas>
    </div>
  );
}
