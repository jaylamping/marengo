import { useEffect, useMemo, useRef } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import { useReducedMotion } from 'motion/react';
import * as THREE from 'three';

/** Particle count — dense enough to read as dust, cheap enough for continuous idle. */
const DUST_COUNT = 360;
/** Soft gray only — amber stays the sole chromatic element in Consul. */
const DUST_COLOR = '#b8c0cc';
/** Near-black void matched to Launch Day `--background`. */
const VOID_COLOR = '#0a0b0f';
const FADE_IN_PER_SEC = 0.35;
const TARGET_OPACITY = 0.7;
/** Drift speed scale (world units / sec). Subliminal, never watched. */
const DRIFT_SCALE = 0.0045;
/** Half-extents of the dust volume around the origin. */
const VOLUME = { x: 8, y: 5, z: 6 } as const;

function makeSoftCircleTexture(): THREE.CanvasTexture {
  const size = 64;
  const canvas = document.createElement('canvas');
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext('2d');
  if (!ctx) {
    // Fallback: empty texture; PointsMaterial still draws square points.
    return new THREE.CanvasTexture(canvas);
  }
  const gradient = ctx.createRadialGradient(
    size / 2,
    size / 2,
    0,
    size / 2,
    size / 2,
    size / 2,
  );
  gradient.addColorStop(0, 'rgba(255,255,255,1)');
  gradient.addColorStop(0.35, 'rgba(255,255,255,0.55)');
  gradient.addColorStop(1, 'rgba(255,255,255,0)');
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, size, size);
  const texture = new THREE.CanvasTexture(canvas);
  texture.needsUpdate = true;
  return texture;
}

function seedDust(count: number): {
  positions: Float32Array;
  velocities: Float32Array;
  baseSizes: Float32Array;
} {
  const positions = new Float32Array(count * 3);
  const velocities = new Float32Array(count * 3);
  const baseSizes = new Float32Array(count);
  for (let i = 0; i < count; i += 1) {
    const i3 = i * 3;
    positions[i3] = (Math.random() * 2 - 1) * VOLUME.x;
    positions[i3 + 1] = (Math.random() * 2 - 1) * VOLUME.y;
    positions[i3 + 2] = (Math.random() * 2 - 1) * VOLUME.z;
    // Mostly upward/sideways drift — like dust in still air, not rain.
    velocities[i3] = (Math.random() * 2 - 1) * DRIFT_SCALE;
    velocities[i3 + 1] = (Math.random() * 0.6 + 0.2) * DRIFT_SCALE;
    velocities[i3 + 2] = (Math.random() * 2 - 1) * DRIFT_SCALE * 0.5;
    baseSizes[i] = 0.02 + Math.random() * 0.07;
  }
  return { positions, velocities, baseSizes };
}

/**
 * Dark dust backdrop: near-black void with soft floating particles.
 * Single sanctioned ambient motion in Consul — telemetry chrome stays rock solid.
 * Inspired by AA-VFX “Dark Dust Moving Background” (procedural, no stock asset).
 */
export function DustBackdrop() {
  const scene = useThree((s) => s.scene);
  const reducedMotion = useReducedMotion();
  const pointsRef = useRef<THREE.Points>(null);
  const materialRef = useRef<THREE.PointsMaterial>(null);
  const opacity = useRef(0);
  const documentHidden = useRef(
    typeof document !== 'undefined' ? document.hidden : false,
  );
  const dust = useMemo(() => seedDust(DUST_COUNT), []);
  const map = useMemo(() => makeSoftCircleTexture(), []);

  useEffect(() => {
    const previous = scene.background;
    scene.background = new THREE.Color(VOID_COLOR);
    return () => {
      scene.background = previous;
      map.dispose();
    };
  }, [scene, map]);

  useEffect(() => {
    const onVisibility = () => {
      documentHidden.current = document.hidden;
    };
    document.addEventListener('visibilitychange', onVisibility);
    return () => document.removeEventListener('visibilitychange', onVisibility);
  }, []);

  useFrame((_, delta) => {
    const material = materialRef.current;
    const points = pointsRef.current;
    if (!material || !points) return;

    if (documentHidden.current) {
      return;
    }

    if (opacity.current < TARGET_OPACITY) {
      opacity.current = Math.min(
        TARGET_OPACITY,
        opacity.current + FADE_IN_PER_SEC * delta,
      );
      material.opacity = opacity.current;
    }

    if (reducedMotion) return;

    const pos = points.geometry.attributes.position as THREE.BufferAttribute;
    const arr = pos.array as Float32Array;
    const { velocities } = dust;
    const dt = Math.min(delta, 0.05);

    for (let i = 0; i < DUST_COUNT; i += 1) {
      const i3 = i * 3;
      arr[i3] += velocities[i3] * dt * 60;
      arr[i3 + 1] += velocities[i3 + 1] * dt * 60;
      arr[i3 + 2] += velocities[i3 + 2] * dt * 60;

      // Toroidal wrap so density stays constant.
      if (arr[i3] > VOLUME.x) arr[i3] = -VOLUME.x;
      else if (arr[i3] < -VOLUME.x) arr[i3] = VOLUME.x;
      if (arr[i3 + 1] > VOLUME.y) arr[i3 + 1] = -VOLUME.y;
      else if (arr[i3 + 1] < -VOLUME.y) arr[i3 + 1] = VOLUME.y;
      if (arr[i3 + 2] > VOLUME.z) arr[i3 + 2] = -VOLUME.z;
      else if (arr[i3 + 2] < -VOLUME.z) arr[i3 + 2] = VOLUME.z;
    }
    pos.needsUpdate = true;
  });

  // Per-particle size via material.size is uniform; approximate depth variety
  // with sizeAttenuation + a wide Z volume. Keep one draw call.
  const avgSize =
    dust.baseSizes.reduce((sum, s) => sum + s, 0) / dust.baseSizes.length;

  return (
    <points ref={pointsRef} frustumCulled={false}>
      <bufferGeometry>
        <bufferAttribute
          attach="attributes-position"
          args={[dust.positions, 3]}
        />
      </bufferGeometry>
      <pointsMaterial
        ref={materialRef}
        map={map}
        color={DUST_COLOR}
        size={avgSize}
        sizeAttenuation
        transparent
        opacity={0}
        depthWrite={false}
        blending={THREE.AdditiveBlending}
        toneMapped={false}
      />
    </points>
  );
}
