import * as React from 'react';
import { useTestingStore } from '@/state/testingStore';
import { robotInventory } from '@/data/robot-inventory';
import { Checkbox } from '@/components/ui/checkbox';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { cn } from '@/lib/utils';
import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';

export function ActuatorMultiSelect() {
  const { selectedJointNames, selectJoint, deselectJoint } = useTestingStore();
  const actuators = robotInventory.filter((item) => item.kind === 'actuator');

  const toggleJoint = (name: string, isSelected: boolean) => {
    if (isSelected) {
      selectJoint(name);
    } else {
      deselectJoint(name);
    }
  };

  return (
    <Card className={dashboardPanelCardClassName}>
      <CardHeader>
        <CardTitle>Actuators ({selectedJointNames.length} / {actuators.length} selected)</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 gap-2">
          {actuators.map((actuator) => {
            const isSelected = selectedJointNames.includes(actuator.name);
            return (
              <div key={actuator.id} className="flex items-center gap-2 p-2 rounded hover:bg-muted/50">
                <Checkbox
                  checked={isSelected}
                  onCheckedChange={(checked) => toggleJoint(actuator.name, !!checked)}
                />
                <span className="text-sm flex-1">{actuator.name}</span>
                <Badge variant={actuator.status === 'Enabled' ? 'default' : 'outline'}>
                  {actuator.status}
                </Badge>
              </div>
            );
          })}
        </div>
      </CardContent>
    </Card>
  );
}
