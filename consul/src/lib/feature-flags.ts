/** Consul feature gates (Vite env). */

export function isActuatorsFeatureEnabled(): boolean {
  return import.meta.env.VITE_FEATURE_ACTUATORS === 'true';
}
