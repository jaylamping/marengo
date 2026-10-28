import * as React from 'react';
import { useTestingStore } from '@/state/testingStore';
import { useRobotStore } from '@/state/robotStore';
import {
  INVENTORY_GROUP_ORDER,
  INVENTORY_GROUP_LABELS,
  inventoryGroupForJoint,
  type InventoryGroup,
} from '@/data/robot-inventory';
import { useConfigSnapshot } from '@/hooks/use-config-snapshot';
import { Checkbox } from '@/components/ui/checkbox';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';

/**
 * Joints offered for Testing are the gateway's command-eligible joints (the
 * config snapshot's `joints`), never the static catalog: an unwired catalog
 * joint makes the gateway refuse the whole batch (UI-13).
 */
export function ActuatorMultiSelect() {
  const { selectedJointNames, selectJoint, deselectJoint } = useTestingStore();
  const robotState = useRobotStore((s) => s.robotState);
  const { data: config = null } = useConfigSnapshot();
  const joints = React.useMemo(() => config?.joints ?? [], [config]);
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

  const groupedJoints = React.useMemo(() => {
    const groups = {} as Record<InventoryGroup, string[]>;
    INVENTORY_GROUP_ORDER.forEach(g => groups[g] = []);
    joints.forEach(name => groups[inventoryGroupForJoint(name)].push(name));
    return groups;
  }, [joints]);

  // Auto-open groups that have selected items
  React.useEffect(() => {
    const newOpenGroups = { ...openGroups };
    let changed = false;
    INVENTORY_GROUP_ORDER.forEach(groupKey => {
      const hasSelected = groupedJoints[groupKey].some(name => selectedJointNames.includes(name));
      if (hasSelected && !newOpenGroups[groupKey]) {
        newOpenGroups[groupKey] = true;
        changed = true;
      }
    });
    if (changed) {
      setOpenGroups(newOpenGroups);
    }
  }, [selectedJointNames, groupedJoints]);

  const liveStatus = (name: string): string => {
    const joint = robotState?.joints.find((entry) => entry.name === name);
    if (!joint) return 'No data';
    return joint.driveActive ? 'Enabled' : 'Disabled';
  };

  return (
    <Card variant="panel" className={dashboardPanelCardClassName}>
      <CardHeader className="pb-3">
        <CardTitle>Actuators ({selectedJointNames.length} / {joints.length} selected)</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2">
        {joints.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            Joint list unavailable: the gateway config snapshot has not loaded.
          </p>
        ) : null}
        {INVENTORY_GROUP_ORDER.map((groupKey) => {
          const groupJoints = groupedJoints[groupKey];
          if (groupJoints.length === 0) return null;

          const selectedInGroup = groupJoints.filter(name => selectedJointNames.includes(name)).length;
          const isAllSelected = selectedInGroup === groupJoints.length;
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
                  <Badge variant="secondary" className="ml-2 text-xs font-normal">{selectedInGroup}/{groupJoints.length}</Badge>
                </button>
                <Checkbox
                  checked={isAllSelected ? true : isIndeterminate ? 'indeterminate' : false}
                  onCheckedChange={(checked) => {
                    groupJoints.forEach(name => {
                      if (checked) selectJoint(name);
                      else deselectJoint(name);
                    });
                  }}
                />
              </div>
              {isOpen && (
                <div className="p-2 space-y-1 bg-background/50">
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-x-4 gap-y-1">
                    {groupJoints.map((name) => {
                      const isSelected = selectedJointNames.includes(name);
                      const status = liveStatus(name);
                      return (
                        <div key={name} className="flex items-center gap-2 p-1.5 rounded hover:bg-muted/50">
                          <Checkbox
                            checked={isSelected}
                            onCheckedChange={(checked) => toggleJoint(name, !!checked)}
                          />
                          <span className="text-sm flex-1 truncate" title={name}>{name}</span>
                          <Badge variant={status === 'Enabled' ? 'default' : 'outline'} className="text-[10px] px-1.5 py-0 font-normal">
                            {status}
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
