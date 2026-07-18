import * as React from 'react';
import { useTestingStore } from '@/state/testingStore';
import { robotInventory, INVENTORY_GROUP_ORDER, INVENTORY_GROUP_LABELS } from '@/data/robot-inventory';
import { Checkbox } from '@/components/ui/checkbox';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { dashboardGlassCardClassName } from '@/components/dashboard/layout/constants';

export function ActuatorMultiSelect() {
  const { selectedJointNames, selectJoint, deselectJoint } = useTestingStore();
  const actuators = robotInventory.filter((item) => item.kind === 'actuator');
  const [openGroups, setOpenGroups] = React.useState<Record<string, boolean>>({
    right_arm: true,
  });

  const toggleJoint = (name: string, isSelected: boolean) => {
    if (isSelected) {
      selectJoint(name);
    } else {
      deselectJoint(name);
    }
  };

  const toggleGroup = (groupKey: string) => {
    setOpenGroups(prev => ({ ...prev, [groupKey]: !prev[groupKey] }));
  };

  const groupedActuators = React.useMemo(() => {
    const groups: Record<string, typeof actuators> = {};
    INVENTORY_GROUP_ORDER.forEach(g => groups[g] = []);
    actuators.forEach(a => {
      if (groups[a.group]) {
        groups[a.group].push(a);
      }
    });
    return groups;
  }, [actuators]);

  // Auto-open groups that have selected items
  React.useEffect(() => {
    const newOpenGroups = { ...openGroups };
    let changed = false;
    INVENTORY_GROUP_ORDER.forEach(groupKey => {
      const groupActuators = groupedActuators[groupKey];
      if (groupActuators) {
        const hasSelected = groupActuators.some(a => selectedJointNames.includes(a.name));
        if (hasSelected && !newOpenGroups[groupKey]) {
          newOpenGroups[groupKey] = true;
          changed = true;
        }
      }
    });
    if (changed) {
      setOpenGroups(newOpenGroups);
    }
  }, [selectedJointNames, groupedActuators]);

  return (
    <Card className={dashboardGlassCardClassName}>
      <CardHeader className="pb-3">
        <CardTitle>Actuators ({selectedJointNames.length} / {actuators.length} selected)</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2">
        {INVENTORY_GROUP_ORDER.map((groupKey) => {
          const groupActuators = groupedActuators[groupKey];
          if (!groupActuators || groupActuators.length === 0) return null;
          
          const selectedInGroup = groupActuators.filter(a => selectedJointNames.includes(a.name)).length;
          const isAllSelected = selectedInGroup === groupActuators.length;
          const isIndeterminate = selectedInGroup > 0 && !isAllSelected;
          const isOpen = openGroups[groupKey];

          return (
            <div key={groupKey} className="border border-border/50 rounded-md overflow-hidden">
              <div className="flex items-center gap-2 py-1.5 px-2 bg-muted/30">
                <button 
                  className="flex items-center gap-1 hover:bg-muted/50 p-1 rounded flex-1 text-left"
                  onClick={() => toggleGroup(groupKey)}
                >
                  <span className={`transition-transform duration-200 inline-block w-4 text-center ${isOpen ? 'rotate-90' : ''}`}>
                    ▶
                  </span>
                  <span className="font-medium text-sm">{INVENTORY_GROUP_LABELS[groupKey]}</span>
                  <Badge variant="secondary" className="ml-2 text-xs font-normal">{selectedInGroup}/{groupActuators.length}</Badge>
                </button>
                <Checkbox
                  checked={isAllSelected ? true : isIndeterminate ? 'indeterminate' : false}
                  onCheckedChange={(checked) => {
                    groupActuators.forEach(a => {
                      if (checked) selectJoint(a.name);
                      else deselectJoint(a.name);
                    });
                  }}
                />
              </div>
              {isOpen && (
                <div className="p-2 space-y-1 bg-background/50">
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-x-4 gap-y-1">
                    {groupActuators.map((actuator) => {
                      const isSelected = selectedJointNames.includes(actuator.name);
                      return (
                        <div key={actuator.id} className="flex items-center gap-2 p-1.5 rounded hover:bg-muted/50">
                          <Checkbox
                            checked={isSelected}
                            onCheckedChange={(checked) => toggleJoint(actuator.name, !!checked)}
                          />
                          <span className="text-sm flex-1 truncate" title={actuator.name}>{actuator.name}</span>
                          <Badge variant={actuator.status === 'Enabled' ? 'default' : 'outline'} className="text-[10px] px-1.5 py-0 font-normal">
                            {actuator.status}
                          </Badge>
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </CardContent>
    </Card>
  );
}
