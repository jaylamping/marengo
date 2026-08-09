/**
 * PROTOTYPE — React mount point for the vanilla Three.js picker.
 *
 * The scene is built once and driven imperatively; React state changes never
 * rebuild the WebGL context.
 */
import { useEffect, useRef } from 'react';

import { cn } from '@/lib/utils';

import { HumanoidScene, type SceneStyle } from './humanoid-scene';
import { PROTO_JOINTS, type Limb } from './mock-hardware';

export function HumanoidViewport({
  className,
  selectedId,
  focusLimb = null,
  style = 'solid',
  orbit = true,
  ground = true,
  onSelect,
}: {
  className?: string;
  selectedId: string | null;
  focusLimb?: Limb | null;
  style?: SceneStyle;
  orbit?: boolean;
  ground?: boolean;
  onSelect: (id: string | null) => void;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<HumanoidScene | null>(null);
  const onSelectRef = useRef(onSelect);
  onSelectRef.current = onSelect;

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const scene = new HumanoidScene({
      container,
      joints: PROTO_JOINTS,
      style,
      orbit,
      ground,
      onSelect: (id) => onSelectRef.current(id),
    });
    sceneRef.current = scene;

    return () => {
      scene.dispose();
      sceneRef.current = null;
    };
  }, [style, orbit, ground]);

  useEffect(() => {
    sceneRef.current?.setSelected(selectedId);
  }, [selectedId]);

  useEffect(() => {
    sceneRef.current?.setFocusLimb(focusLimb);
  }, [focusLimb]);

  return <div ref={containerRef} className={cn('relative', className)} />;
}
