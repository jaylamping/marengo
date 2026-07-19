import type { RobotState } from '@/gen/marengo/v1/marengo_pb';
import type { TeachSample } from '@/lib/teach-record';

type Listener = (sample: TeachSample) => void;

const listeners = new Set<Listener>();

/** Unthrottled RobotState → teach samples (subscribe before UI throttle). */
export function subscribeTeachSamples(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function publishTeachSampleFromRobotState(state: RobotState): void {
  if (listeners.size === 0) return;
  const q: Record<string, number> = {};
  for (const j of state.joints) {
    q[j.name] = j.position;
  }
  const sample: TeachSample = {
    tMs: Number(state.timestampMs),
    q,
  };
  for (const listener of listeners) {
    listener(sample);
  }
}
