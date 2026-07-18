import { useEffect, useRef, useState } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import { useReducedMotion } from 'motion/react';
import * as THREE from 'three';
import { EXRLoader } from 'three/examples/jsm/loaders/EXRLoader.js';

const STARMAP_URL = '/textures/starmap_2020_4k.exr';

/** Dim the void so amber stays the only chromatic element in the app. */
const BACKGROUND_INTENSITY = 0.32;
/** One full revolution every ~35 minutes — subliminal, never watched. */
const ROTATION_RAD_PER_SEC = (Math.PI * 2) / 2100;
const FADE_IN_PER_SEC = 0.25;

/**
 * Deep-space backdrop: NASA SVS Deep Star Map 2020 (Gaia DR2) as the scene
 * background with a slow, continuous drift. This is the single sanctioned
 * ambient motion in Consul — the robot (telemetry) stays rock solid.
 * Credit: NASA/GSFC SVS. Gaia DR2: ESA/Gaia/DPAC.
 */
export function SpaceBackdrop() {
  const scene = useThree((s) => s.scene);
  const reducedMotion = useReducedMotion();
  const [texture, setTexture] = useState<THREE.DataTexture | null>(null);
  const intensity = useRef(0);

  useEffect(() => {
    let cancelled = false;
    const loader = new EXRLoader();
    loader.load(STARMAP_URL, (loaded) => {
      if (cancelled) {
        loaded.dispose();
        return;
      }
      loaded.mapping = THREE.EquirectangularReflectionMapping;
      setTexture(loaded);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!texture) return;
    intensity.current = 0;
    scene.background = texture;
    scene.backgroundIntensity = 0;
    return () => {
      scene.background = null;
      scene.backgroundIntensity = 1;
      texture.dispose();
    };
  }, [scene, texture]);

  useFrame((_, delta) => {
    if (!texture) return;
    if (intensity.current < BACKGROUND_INTENSITY) {
      intensity.current = Math.min(
        BACKGROUND_INTENSITY,
        intensity.current + FADE_IN_PER_SEC * delta,
      );
      scene.backgroundIntensity = intensity.current;
    }
    if (!reducedMotion) {
      scene.backgroundRotation.y += ROTATION_RAD_PER_SEC * delta;
    }
  });

  return null;
}
