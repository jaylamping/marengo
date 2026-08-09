/**
 * PROTOTYPE — clickable stick humanoid (not real URDF meshes).
 */
import { Canvas } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { useMemo } from 'react';

import { PROTO_JOINTS, type ProtoJoint } from './mock-hardware';

function JointOrb({
  joint,
  selected,
  onSelect,
}: {
  joint: ProtoJoint;
  selected: boolean;
  onSelect: (id: string) => void;
}) {
  const color = selected
    ? '#fbbf24'
    : joint.completenessWarn
      ? '#f97316'
      : joint.onCan
        ? '#5eead4'
        : '#64748b';

  return (
    <mesh
      position={joint.pos}
      onClick={(e) => {
        e.stopPropagation();
        onSelect(joint.id);
      }}
      onPointerOver={(e) => {
        e.stopPropagation();
        document.body.style.cursor = 'pointer';
      }}
      onPointerOut={() => {
        document.body.style.cursor = 'default';
      }}
    >
      <sphereGeometry args={[selected ? 0.08 : 0.06, 16, 16]} />
      <meshStandardMaterial
        color={color}
        emissive={color}
        emissiveIntensity={selected ? 0.55 : 0.2}
      />
    </mesh>
  );
}

function BodyBones() {
  // rough limbs as thin boxes for presence
  const bones: { pos: [number, number, number]; size: [number, number, number] }[] =
    [
      { pos: [0, 1.15, 0], size: [0.22, 0.45, 0.12] }, // torso
      { pos: [0, 1.5, 0], size: [0.14, 0.14, 0.14] }, // head
      { pos: [0.4, 1.15, 0], size: [0.45, 0.06, 0.06] }, // r upper
      { pos: [0.55, 0.95, 0.04], size: [0.06, 0.35, 0.06] }, // r lower
      { pos: [-0.4, 1.15, 0], size: [0.45, 0.06, 0.06] },
      { pos: [-0.55, 0.95, 0.04], size: [0.06, 0.35, 0.06] },
      { pos: [0.12, 0.55, 0], size: [0.08, 0.45, 0.08] },
      { pos: [-0.12, 0.55, 0], size: [0.08, 0.45, 0.08] },
    ];
  return (
    <group>
      {bones.map((b, i) => (
        <mesh key={i} position={b.pos}>
          <boxGeometry args={b.size} />
          <meshStandardMaterial color="#334155" roughness={0.85} />
        </mesh>
      ))}
    </group>
  );
}

export function StickHumanoidCanvas({
  selectedId,
  onSelect,
  className,
  autoRotate = false,
}: {
  selectedId: string | null;
  onSelect: (id: string) => void;
  className?: string;
  autoRotate?: boolean;
}) {
  const joints = useMemo(() => PROTO_JOINTS, []);

  return (
    <div className={className}>
      <Canvas camera={{ position: [1.6, 1.4, 2.2], fov: 42 }} dpr={[1, 1.75]}>
        <color attach="background" args={['#0b0f14']} />
        <ambientLight intensity={0.55} />
        <directionalLight position={[3, 5, 2]} intensity={1.1} />
        <BodyBones />
        {joints.map((j) => (
          <JointOrb
            key={j.id}
            joint={j}
            selected={selectedId === j.id}
            onSelect={onSelect}
          />
        ))}
        <OrbitControls
          enablePan={false}
          minDistance={1.2}
          maxDistance={4}
          target={[0, 1.0, 0]}
          autoRotate={autoRotate}
          autoRotateSpeed={0.6}
        />
      </Canvas>
    </div>
  );
}
