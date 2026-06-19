import { useState } from 'react';

import { memorySearchPanelShellClassName } from '@/components/dashboard/memory/constants';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { searchMemories } from '@/lib/mem0-api';
import type { Mem0Memory } from '@/lib/mem0-config';

type MemorySearchPanelProps = {
  onResults: (rows: Mem0Memory[]) => void;
  onClear: () => void;
};

export function MemorySearchPanel({ onResults, onClear }: MemorySearchPanelProps) {
  const [query, setQuery] = useState('');
  const [loading, setLoading] = useState(false);

  async function handleSearch() {
    if (!query.trim()) {
      return;
    }
    setLoading(true);
    const rows = await searchMemories(query.trim());
    onResults(rows);
    setLoading(false);
  }

  return (
    <div className={memorySearchPanelShellClassName} data-testid="memory-search-panel-shell">
      <Input
        variant="glass"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Semantic search mem0…"
        className="max-w-md"
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            void handleSearch();
          }
        }}
      />
      <Button type="button" onClick={() => void handleSearch()} disabled={loading}>
        {loading ? 'Searching…' : 'Search'}
      </Button>
      <Button type="button" variant="outline" onClick={onClear}>
        Clear
      </Button>
    </div>
  );
}
