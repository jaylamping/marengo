import { useEffect, useRef } from 'react';

import { BenchArmScene } from '@/components/dashboard/hardware/bench-arm-scene';
import type { HardwareJointRow } from '@/components/dashboard/hardware/build-hardware-rows';
import { cn } from '@/lib/utils';

type Hardware3dViewProps = {
  className?: string;
  rows: HardwareJointRow[];
  selectedJoint: string | null;
  onSelect: (joint: string | null) => void;
};

export function Hardware3dView({
  className,
  rows,
  selectedJoint,
  onSelect,
}: Hardware3dViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<BenchArmScene | null>(null);
  const onSelectRef = useRef(onSelect);
  onSelectRef.current = onSelect;

  useEffect(() => {
    const container = containerRef.current;
    if (!container || rows.length === 0) {
      return;
    }

    const scene = new BenchArmScene({
      container,
      rows,
      onSelect: (joint) => onSelectRef.current(joint),
    });
    sceneRef.current = scene;

    return () => {
      scene.dispose();
      sceneRef.current = null;
    };
  }, [rows]);

  useEffect(() => {
    sceneRef.current?.setSelected(selectedJoint);
  }, [selectedJoint]);

  return (
    <div
      ref={containerRef}
      className={cn('relative min-h-[280px] pointer-events-auto rounded-sm border border-line', className)}
      data-testid="hardware-3d-view"
    />
  );
}
