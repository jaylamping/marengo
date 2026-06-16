import { useCallback, useEffect, useMemo, useState } from 'react';

import { dashboardLogsClassName } from '@/components/dashboard/layout/constants';
import { MemoryConnectionBanner } from '@/components/dashboard/memory/memory-connection-banner';
import { MemoryDetailSheet } from '@/components/dashboard/memory/memory-detail-sheet';
import { MemoryNamespaceTabs } from '@/components/dashboard/memory/memory-namespace-tabs';
import { MemorySearchPanel } from '@/components/dashboard/memory/memory-search-panel';
import { MemoryTimelineChart } from '@/components/dashboard/memory/memory-timeline-chart';
import { MemoryVirtualList } from '@/components/dashboard/memory/memory-virtual-list';
import { Button } from '@/components/ui/button';
import { fetchMemories, fetchMemoryHistory, pingMem0 } from '@/lib/mem0-api';
import { mem0PollMs, mem0UserId, type Mem0Memory, type Mem0Namespace } from '@/lib/mem0-config';

export function MemoryOverview() {
  const [reachable, setReachable] = useState<boolean | null>(null);
  const [memories, setMemories] = useState<Mem0Memory[]>([]);
  const [searchResults, setSearchResults] = useState<Mem0Memory[] | null>(null);
  const [namespace, setNamespace] = useState<Mem0Namespace>('all');
  const [selected, setSelected] = useState<Mem0Memory | null>(null);
  const [history, setHistory] = useState<Awaited<ReturnType<typeof fetchMemoryHistory>>>([]);
  const [detailOpen, setDetailOpen] = useState(false);

  const refresh = useCallback(async () => {
    const ok = await pingMem0();
    setReachable(ok);
    if (!ok) {
      setMemories([]);
      return;
    }
    setMemories(await fetchMemories());
  }, []);

  useEffect(() => {
    void refresh();
    const pollMs = mem0PollMs();
    if (pollMs <= 0) {
      return;
    }
    const timer = window.setInterval(() => {
      void refresh();
    }, pollMs);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const counts = useMemo(() => {
    const tally: Record<string, number> = { all: memories.length };
    for (const memory of memories) {
      tally[memory.namespace] = (tally[memory.namespace] ?? 0) + 1;
    }
    return tally;
  }, [memories]);

  const visible = useMemo(() => {
    const source = searchResults ?? memories;
    if (namespace === 'all') {
      return source;
    }
    return source.filter((m) => m.namespace === namespace);
  }, [memories, namespace, searchResults]);

  async function handleSelect(memory: Mem0Memory) {
    setSelected(memory);
    setHistory(await fetchMemoryHistory(memory.id));
    setDetailOpen(true);
  }

  return (
    <div className={dashboardLogsClassName}>
      <MemoryConnectionBanner reachable={reachable} />
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-sm text-muted-foreground">
          mem0 · {mem0UserId()} · read-only
        </p>
        <Button type="button" variant="outline" size="sm" onClick={() => void refresh()}>
          Refresh
        </Button>
      </div>
      <MemoryTimelineChart memories={memories} />
      <MemorySearchPanel
        onResults={setSearchResults}
        onClear={() => setSearchResults(null)}
      />
      <MemoryNamespaceTabs value={namespace} counts={counts} onChange={setNamespace} />
      <MemoryVirtualList memories={visible} selectedId={selected?.id ?? null} onSelect={handleSelect} />
      <MemoryDetailSheet
        memory={selected}
        history={history}
        open={detailOpen}
        onOpenChange={setDetailOpen}
      />
    </div>
  );
}
