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
  const rowsRef = useRef(rows);
  rowsRef.current = rows;
  const selectedRef = useRef(selectedJoint);
  selectedRef.current = selectedJoint;

  // Rows are rebuilt on every RobotState (10 Hz). The scene is rebuilt only when
  // the joint set changes; otherwise markers are recoloured in place, so the
  // camera and WebGL context survive and the selection stays applied (UI-16).
  const jointKey = rows.map((row) => row.joint).join('\n');

  useEffect(() => {
    const container = containerRef.current;
    if (!container || jointKey === '') {
      return;
    }

    const scene = new BenchArmScene({
      container,
      rows: rowsRef.current,
      onSelect: (joint) => onSelectRef.current(joint),
    });
    scene.setSelected(selectedRef.current);
    sceneRef.current = scene;

    return () => {
      scene.dispose();
      sceneRef.current = null;
    };
  }, [jointKey]);

  useEffect(() => {
    sceneRef.current?.updateRows(rows);
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
