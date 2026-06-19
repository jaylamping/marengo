import { logsTabsVariant } from '@/components/dashboard/logs/constants';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';

export type LogsMode = 'live' | 'archive' | 'can';

type Props = {
  mode: LogsMode;
  onModeChange: (mode: LogsMode) => void;
};

export function LogsModeTabs({ mode, onModeChange }: Props) {
  return (
    <Tabs value={mode} onValueChange={(v) => onModeChange(v as LogsMode)}>
      <TabsList variant={logsTabsVariant}>
        <TabsTrigger variant={logsTabsVariant} value="live">
          Live
        </TabsTrigger>
        <TabsTrigger variant={logsTabsVariant} value="archive">
          Archive
        </TabsTrigger>
        <TabsTrigger variant={logsTabsVariant} value="can">
          CAN
        </TabsTrigger>
      </TabsList>
    </Tabs>
  );
}
