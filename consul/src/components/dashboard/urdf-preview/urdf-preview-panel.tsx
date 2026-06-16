import { useRef, useMemo, useCallback } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import * as THREE from 'three';

import { useRobotModel } from '@/urdf/RobotModelContext';
import { computeForwardKinematics } from '@/urdf/forward-kinematics';
import { useRobotStore } from '@/state/robotStore';
import type { RobotModel, UrdfGeometry, VisualSpec } from '@/urdf/parse-urdf';

function geometryFromSpec(geometry: UrdfGeometry): THREE.BufferGeometry {
  switch (geometry.type) {
    case 'box': {
      const [x, y, z] = geometry.size;
      return new THREE.BoxGeometry(x, y, z);
    }
    case 'cylinder': {
      // Three.js CylinderGeometry is aligned with Y; URDF cylinders are aligned with Z.
      const geo = new THREE.CylinderGeometry(geometry.radius, geometry.radius, geometry.length, 32);
      geo.rotateX(Math.PI / 2);
      return geo;
    }
    case 'sphere':
      return new THREE.SphereGeometry(geometry.radius, 32, 32);
    default:
      return new THREE.BoxGeometry(0.1, 0.1, 0.1);
  }
}

function colorFromSpec(visual: VisualSpec, fallback: string): string {
  if (visual.material?.color) {
    const [r, g, b, a] = visual.material.color;
    return `rgba(${(r * 255) | 0}, ${(g * 255) | 0}, ${(b * 255) | 0}, ${a ?? 1})`;
  }
  return fallback;
}

function visualOriginMatrix(origin: VisualSpec['origin']): THREE.Matrix4 {
  const matrix = new THREE.Matrix4();
  const [ox, oy, oz] = origin.xyz;
  matrix.makeTranslation(ox, oy, oz);
  const [rx, ry, rz] = origin.rpy;
  const euler = new THREE.Euler(rx, ry, rz, 'XYZ');
  matrix.multiply(new THREE.Matrix4().makeRotationFromEuler(euler));
  return matrix;
}

function LinkVisual({
  visual,
  fallbackColor,
}: {
  visual: VisualSpec;
  fallbackColor: string;
}) {
  const geometry = useMemo(() => geometryFromSpec(visual.geometry), [visual.geometry]);
  const matrix = useMemo(() => visualOriginMatrix(visual.origin), [visual.origin]);
  const color = colorFromSpec(visual, fallbackColor);

  return (
    <mesh geometry={geometry} matrix={matrix} matrixAutoUpdate={false}>
      <meshStandardMaterial color={color} roughness={0.5} metalness={0.2} />
    </mesh>
  );
}

function UrdfScene() {
  const model = useRobotModel();
  const groupRef = useRef<THREE.Group>(null);
  const linkGroupRefs = useRef<Map<string, THREE.Group>>(new Map());

  const registerLinkGroup = useCallback(
    (name: string) =>
      (node: THREE.Group | null): void => {
        if (node) {
          linkGroupRefs.current.set(name, node);
        }
      },
    [],
  );

  const linkChain = useMemo(() => {
    const links: { name: string; visuals: VisualSpec[]; color: string }[] = [];
    const visited = new Set<string>();

    for (const rootName of model.rootLinks) {
      const queue: { name: string }[] = [{ name: rootName }];
      while (queue.length > 0) {
        const { name } = queue.shift()!;
        if (visited.has(name)) continue;
        visited.add(name);

        const link = model.links.get(name);
        links.push({
          name,
          visuals: link?.visuals ?? [],
          color: name === 'base_link' ? '#94a3b8' : '#3b82f6',
        });

        for (const joint of model.joints.values()) {
          if (joint.parent === name && !visited.has(joint.child)) {
            queue.push({ name: joint.child });
          }
        }
      }
    }
    return links;
  }, [model]);

  useFrame(() => {
    if (!groupRef.current) return;

    const state = useRobotStore.getState();
    const positions: Record<string, number> = {};
    for (const jointName of model.joints.keys()) {
      const points = state.trackingPointsByJoint[jointName];
      positions[jointName] = points && points.length > 0 ? points[points.length - 1].measured : 0;
    }

    const matrices = computeForwardKinematics(model, positions);
    for (const { name } of linkChain) {
      const group = linkGroupRefs.current.get(name);
      const mat = matrices.get(name);
      if (group && mat) {
        group.matrix.copy(mat);
      }
    }
  });

  return (
    <group ref={groupRef}>
      <axesHelper args={[0.5]} />
      {linkChain.map(({ name, visuals, color }) => (
        <group key={name} ref={registerLinkGroup(name)} matrixAutoUpdate={false}>
          {visuals.length > 0 ? (
            visuals.map((visual, index) => <LinkVisual key={index} visual={visual} fallbackColor={color} />)
          ) : (
            <mesh>
              <boxGeometry args={[0.05, 0.05, 0.05]} />
              <meshStandardMaterial color={color} roughness={0.5} metalness={0.2} />
            </mesh>
          )}
        </group>
      ))}
    </group>
  );
}

export function UrdfPreviewPanel() {
  return (
    <div className="h-[400px] w-full rounded-lg border bg-background">
      <Canvas camera={{ position: [0.5, 0.5, 0.5], fov: 50 }} gl={{ antialias: true }}>
        <ambientLight intensity={0.5} />
        <directionalLight position={[10, 10, 5]} intensity={1} />
        <UrdfScene />
        <OrbitControls makeDefault />
      </Canvas>
    </div>
  );
}
