import * as React from 'react';
import { useTestingStore } from '@/state/testingStore';
import { robotInventory, InventoryGroup } from '@/data/robot-inventory';
import { Button } from '@/components/ui/button';

export function PresetGroupButtons() {
  const { selectJoint, deselectJoint } = useTestingStore();

  const groups: { label: string; filter: (item: any) => boolean }[] = [
    { label: 'Right Arm', filter: (item) => item.group === 'right_arm' },
    { label: 'Left Arm', filter: (item) => item.group === 'left_arm' },
    { label: 'Both Arms', filter: (item) => item.group === 'right_arm' || item.group === 'left_arm' },
    { label: 'Both Elbows', filter: (item) => item.name.includes('elbow') },
    { label: 'Clear All', filter: () => false },
  ];

  const handlePreset = (filter: (item: any) => boolean) => {
    const jointsToSelect = robotInventory.filter(filter).map(i => i.name);
    const allJoints = robotInventory.filter(i => i.kind === 'actuator').map(i => i.name);
    
    allJoints.forEach(name => deselectJoint(name));
    jointsToSelect.forEach(name => selectJoint(name));
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
