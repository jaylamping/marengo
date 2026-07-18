// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import {
  memoryListShellClassName,
  memorySearchPanelShellClassName,
  memorySheetContentClassName,
} from '@/components/dashboard/memory/constants';
import { MemoryDetailSheet } from '@/components/dashboard/memory/memory-detail-sheet';
import { MemoryNamespaceTabs } from '@/components/dashboard/memory/memory-namespace-tabs';
import { MemorySearchPanel } from '@/components/dashboard/memory/memory-search-panel';
import { MemoryVirtualList } from '@/components/dashboard/memory/memory-virtual-list';
import type { Mem0Memory } from '@/lib/mem0-config';

vi.mock('@/lib/mem0-api', () => ({
  searchMemories: vi.fn().mockResolvedValue([]),
}));

vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: () => ({
    getTotalSize: () => 72,
    getVirtualItems: () => [{ index: 0, start: 0, size: 72, key: 'mem-1' }],
  }),
}));

afterEach(() => {
  cleanup();
});

describe('memory panel shell constants', () => {
  it('defines chrome-tier search panel shell with pointer-events-auto', () => {
    expect(memorySearchPanelShellClassName).toContain('bg-surface-1');
    expect(memorySearchPanelShellClassName).toContain('pointer-events-auto');
  });

  it('defines data-tier list shell with blur', () => {
    expect(memoryListShellClassName).toContain('bg-surface-1');
    expect(memoryListShellClassName).toContain('pointer-events-auto');
  });

  it('defines sheet content panel without row animation', () => {
    expect(memorySheetContentClassName).toContain('bg-surface-1');
    expect(memorySheetContentClassName).not.toContain('animate-in');
  });
});

describe('MemoryNamespaceTabs panel chrome', () => {
  it('renders namespace tabs with panel tabs list', () => {
    render(
      <MemoryNamespaceTabs value="all" counts={{ all: 3, sdd: 1 }} onChange={() => undefined} />,
    );

    const allTab = screen.getByRole('tab', { name: /All/ });
    expect(allTab.closest('[role="tablist"]')?.className).toContain('bg-surface-1');
    expect(screen.getByRole('tab', { name: /SDD/ })).toBeTruthy();
  });
});

describe('MemorySearchPanel panel chrome', () => {
  it('wraps semantic search controls in the search panel shell', () => {
    render(<MemorySearchPanel onResults={() => undefined} onClear={() => undefined} />);

    const shell = screen.getByTestId('memory-search-panel-shell');
    expect(shell.className).toContain('bg-surface-1');
    const input = screen.getByPlaceholderText('Semantic search mem0…');
    expect(input.className).toContain('bg-surface-1');
    expect(screen.getByRole('button', { name: 'Search' })).toBeTruthy();
  });
});

describe('MemoryVirtualList data-tier shell', () => {
  it('renders memories inside the list shell with opaque rows', () => {
    const memories: Mem0Memory[] = [
      {
        id: 'mem-1',
        memory: 'shoulder pitch limit verified',
        namespace: 'hardware',
        topicKey: 'hardware/shoulder/pitch',
        created_at: '2026-01-01',
        updated_at: '2026-01-02',
      },
    ];

    render(
      <MemoryVirtualList memories={memories} selectedId={null} onSelect={() => undefined} />,
    );

    const shell = screen.getByTestId('memory-list-shell');
    expect(shell.className).toContain('bg-surface-1');
    expect(screen.getByRole('button', { name: /hardware\/shoulder\/pitch/ })).toBeTruthy();
  });
});

describe('MemoryDetailSheet panel sheet', () => {
  it('applies panel sheet content when memory is open', () => {
    const memory: Mem0Memory = {
      id: 'mem-1',
      memory: 'CAN termination verified on bench',
      namespace: 'pi',
      topicKey: 'pi/can/termination',
      created_at: '2026-01-01',
      updated_at: '2026-01-02',
    };

    render(
      <MemoryDetailSheet
        memory={memory}
        history={[]}
        open
        onOpenChange={() => undefined}
      />,
    );

    const content = document.querySelector('[data-slot="sheet-content"]');
    expect(content?.className).toContain('bg-surface-1');
    expect(screen.getByText('CAN termination verified on bench')).toBeTruthy();
  });
});
