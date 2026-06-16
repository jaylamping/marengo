import { useRef, useMemo, useEffect, useCallback } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import * as THREE from 'three';

import { useRobotModel } from '@/urdf/RobotModelContext';
import { computeForwardKinematics } from '@/urdf/forward-kinematics';
import { useRobotStore } from '@/state/robotStore';

const BOX_SIZE: [number, number, number] = [0.1, 0.1, 0.2];

function LinkBox({
  matrix,
  color,
  boxRef,
}: {
  matrix: THREE.Matrix4;
  color: string;
  boxRef: (m: THREE.Mesh | null) => void;
}) {
  const ref = useRef<THREE.Mesh>(null);

  // Update matrix imperatively each frame — no React re-render needed
  useFrame(() => {
    if (ref.current) {
      ref.current.matrix.copy(matrix);
    }
  });

  return (
    <mesh ref={(node) => { ref.current = node; boxRef(node); }} matrixAutoUpdate={false}>
      <boxGeometry args={BOX_SIZE} />
      <meshStandardMaterial color={color} roughness={0.5} metalness={0.2} />
    </mesh>
  );
}

function UrdfScene() {
  const model = useRobotModel();
  const groupRef = useRef<THREE.Group>(null);
  const meshRefs = useRef<Map<string, THREE.Mesh>>(new Map());

  const registerMesh = useCallback((name: string) => (node: THREE.Mesh | null) => {
    if (node) {
      meshRefs.current.set(name, node);
    }
  }, []);

  // Build a flat list of links with parent info so we don't need recursive components
  const linkChain = useMemo(() => {
    const links: { name: string; parent: string | null }[] = [];
    const visited = new Set<string>();

    // Start from root links
    for (const rootName of model.rootLinks) {
      const queue: { name: string; parent: string | null }[] = [{ name: rootName, parent: null }];
      while (queue.length > 0) {
        const { name, parent } = queue.shift()!;
        if (visited.has(name)) continue;
        visited.add(name);
        links.push({ name, parent });

        for (const joint of model.joints.values()) {
          if (joint.parent === name && !visited.has(joint.child)) {
            queue.push({ name: joint.child, parent: name });
          }
        }
      }
    }
    return links;
  }, [model]);

  // Imperatively update all mesh transforms each frame
  useFrame(() => {
    if (!groupRef.current) return;

    // Read current joint positions from store (imperatively, not via React state)
    const state = useRobotStore.getState();
    const positions: Record<string, number> = {};
    for (const jointName of model.joints.keys()) {
      const points = state.trackingPointsByJoint[jointName];
      positions[jointName] = points && points.length > 0 ? points[points.length - 1].measured : 0;
    }

    const matrices = computeForwardKinematics(model, positions);

    for (const { name } of linkChain) {
      const mesh = meshRefs.current.get(name);
      const mat = matrices.get(name);
      if (mesh && mat) {
        mesh.matrix.copy(mat);
      }
    }
  });

  return (
    <group ref={groupRef}>
      <axesHelper args={[0.5]} />
      {linkChain.map(({ name }) => (
        <LinkBox
          key={name}
          matrix={new THREE.Matrix4().identity()}
          color={name === 'base_link' ? '#94a3b8' : '#3b82f6'}
          boxRef={registerMesh(name)}
        />
      ))}
    </group>
  );
}

export function UrdfPreviewPanel() {
  return (
    <div className="h-[400px] w-full rounded-lg border bg-background">
      <Canvas
        camera={{ position: [0.5, 0.5, 0.5], fov: 50 }}
        gl={{ antialias: true }}
      >
        <ambientLight intensity={0.5} />
        <directionalLight position={[10, 10, 5]} intensity={1} />
        <UrdfScene />
        <OrbitControls makeDefault />
      </Canvas>
    </div>
  );
}
