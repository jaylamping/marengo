import * as React from 'react';
import { useTestingStore } from '@/state/testingStore';
import { inventoryGroupForJoint } from '@/data/robot-inventory';
import { useConfigSnapshot } from '@/hooks/use-config-snapshot';
import { Button } from '@/components/ui/button';

/** Quick selections over the command-eligible joints only (UI-13). */
export function PresetGroupButtons() {
  const { selectedJointNames, selectJoint, deselectJoint } = useTestingStore();
  const { data: config = null } = useConfigSnapshot();
  const joints = config?.joints ?? [];

  const groups: { label: string; filter: (name: string) => boolean }[] = [
    { label: 'Right Arm', filter: (name) => inventoryGroupForJoint(name) === 'right_arm' },
    { label: 'Left Arm', filter: (name) => inventoryGroupForJoint(name) === 'left_arm' },
    {
      label: 'Both Arms',
      filter: (name) => ['right_arm', 'left_arm'].includes(inventoryGroupForJoint(name)),
    },
    { label: 'Both Elbows', filter: (name) => name.includes('elbow') },
    { label: 'Clear All', filter: () => false },
  ];

  const handlePreset = (filter: (name: string) => boolean) => {
    [...selectedJointNames].forEach(name => deselectJoint(name));
    joints.filter(filter).forEach(name => selectJoint(name));
  };

  return (
    <div className="flex flex-wrap gap-2">
      {groups.map((group) => (
        <Button key={group.label} variant="outline" size="sm" onClick={() => handlePreset(group.filter)}>
          {group.label}
        </Button>
      ))}
    </div>
  );
}
