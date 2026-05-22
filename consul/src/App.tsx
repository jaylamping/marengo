import { Routes, Route } from 'react-router-dom';
import { Canvas } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { useRobotStore } from './state/robotStore';

function ThreePlaceholder() {
  const jointPosition = useRobotStore((s) => s.jointPosition);

  return (
    <>
      <ambientLight intensity={0.6} />
      <pointLight position={[10, 10, 10]} />

      {/* Simple animated "arm" proxy — this will become the real URDF visualizer */}
      <mesh
        position={[0, Math.sin(Date.now() / 800) * 0.1, 0]}
        rotation={[0, jointPosition * 0.8, 0]}
      >
        <torusGeometry args={[1, 0.35, 16, 48]} />
        <meshStandardMaterial color="#64748b" metalness={0.3} roughness={0.6} />
      </mesh>

      <OrbitControls enableDamping dampingFactor={0.1} />
    </>
  );
}

export default function App() {
  const { setJointPosition, jointPosition } = useRobotStore();

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh' }}>
      {/* Minimal functional top bar */}
      <header
        style={{
          height: 48,
          background: '#1e2937',
          display: 'flex',
          alignItems: 'center',
          padding: '0 16px',
          borderBottom: '1px solid #334155',
          fontSize: 13,
          fontWeight: 500,
          letterSpacing: '0.02em',
        }}
      >
        <div style={{ flex: 1 }}>Consul</div>
        <div style={{ color: '#64748b' }}>Marengo • local</div>
      </header>

      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        {/* Future left sidebar / inspector */}
        <div
          style={{
            width: 280,
            background: '#0f172a',
            borderRight: '1px solid #1e2937',
            padding: 12,
            fontSize: 12,
          }}
        >
          <div style={{ marginBottom: 8, color: '#64748b' }}>JOINTS (placeholder)</div>

          <div style={{ marginBottom: 12 }}>
            <label>
              left_elbow (demo)
              <input
                type="range"
                min={-1.57}
                max={1.57}
                step={0.01}
                value={jointPosition}
                onChange={(e) => setJointPosition(parseFloat(e.target.value))}
                style={{ width: '100%', marginTop: 4 }}
              />
            </label>
            <div style={{ fontSize: 10, color: '#64748b', marginTop: 2 }}>
              {jointPosition.toFixed(2)} rad
            </div>
          </div>

          <div style={{ fontSize: 11, color: '#475569', marginTop: 24 }}>
            This is the base scaffold.<br />
            Real joint data + URDF will live here.
          </div>
        </div>

        {/* Main 3D area — this is the first-class visualizer surface */}
        <div style={{ flex: 1, position: 'relative', background: '#020617' }}>
          <Canvas
            camera={{ position: [0, 2, 6], fov: 50 }}
            style={{ width: '100%', height: '100%' }}
          >
            <ThreePlaceholder />
          </Canvas>

          <div
            style={{
              position: 'absolute',
              bottom: 12,
              right: 12,
              fontSize: 10,
              color: '#475569',
              background: 'rgba(15, 23, 42, 0.8)',
              padding: '2px 6px',
              borderRadius: 2,
            }}
          >
            Orbit • Drag to rotate • Scroll to zoom
          </div>
        </div>
      </div>
    </div>
  );
}