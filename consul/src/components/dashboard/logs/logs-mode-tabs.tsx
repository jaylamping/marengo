import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';

export type LogsMode = 'live' | 'archive' | 'can';

type Props = {
  mode: LogsMode;
  onModeChange: (mode: LogsMode) => void;
};

export function LogsModeTabs({ mode, onModeChange }: Props) {
  return (
    <Tabs value={mode} onValueChange={(v) => onModeChange(v as LogsMode)}>
      <TabsList>
        <TabsTrigger value="live">Live</TabsTrigger>
        <TabsTrigger value="archive">Archive</TabsTrigger>
        <TabsTrigger value="can">CAN</TabsTrigger>
      </TabsList>
    </Tabs>
  );
}
