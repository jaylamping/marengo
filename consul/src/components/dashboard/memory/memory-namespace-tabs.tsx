import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import type { Mem0Namespace } from '@/lib/mem0-config';

const TABS: { id: Mem0Namespace; label: string }[] = [
  { id: 'all', label: 'All' },
  { id: 'sdd', label: 'SDD' },
  { id: 'feasibility', label: 'Feasibility' },
  { id: 'decision', label: 'Decision' },
  { id: 'hardware', label: 'Hardware' },
  { id: 'cad', label: 'CAD' },
  { id: 'pi', label: 'Pi' },
  { id: 'control', label: 'Control' },
  { id: 'software', label: 'Software' },
  { id: 'research', label: 'Research' },
  { id: 'expert', label: 'Expert' },
  { id: 'maintenance', label: 'Maintenance' },
];

type MemoryNamespaceTabsProps = {
  value: Mem0Namespace;
  counts: Record<string, number>;
  onChange: (value: Mem0Namespace) => void;
};

export function MemoryNamespaceTabs({ value, counts, onChange }: MemoryNamespaceTabsProps) {
  return (
    <Tabs value={value} onValueChange={(v) => onChange(v as Mem0Namespace)}>
      <TabsList className="flex h-auto flex-wrap gap-1">
        {TABS.map((tab) => (
          <TabsTrigger key={tab.id} value={tab.id} className="gap-1.5">
            {tab.label}
            <span className="text-xs text-muted-foreground">({counts[tab.id] ?? 0})</span>
          </TabsTrigger>
        ))}
      </TabsList>
    </Tabs>
  );
}
