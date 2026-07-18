// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import {
  logsArchivePanelShellClassName,
  logsSessionListShellClassName,
  logsSheetContentClassName,
  logsTableShellClassName,
  logsToolbarShellClassName,
} from '@/components/dashboard/logs/constants';
import { LogDetailSheet } from '@/components/dashboard/logs/log-detail-sheet';
import { LogsArchiveSearch } from '@/components/dashboard/logs/logs-archive-search';
import { LogsModeTabs } from '@/components/dashboard/logs/logs-mode-tabs';
import { LogsSessionList } from '@/components/dashboard/logs/logs-session-list';
import { LogsToolbar } from '@/components/dashboard/logs/logs-toolbar';
import { LogsVirtualTable } from '@/components/dashboard/logs/logs-virtual-table';
import { LogsFilterProvider } from '@/components/dashboard/logs/logs-filter-context';
import type { LogEntry } from '@/data/logs';

vi.mock('@/components/dashboard/logs/hooks/use-log-controls', () => ({
  useLogPaused: () => false,
  useLogActions: () => ({ setPaused: vi.fn(), clear: vi.fn() }),
  useLogBufferSnapshot: () => ({
    count: 12,
    levelCounts: { WARN: 1, ERROR: 0, DEBUG: 0, INFO: 11, FATAL: 0 },
  }),
}));

vi.mock('@/components/dashboard/logs/hooks/use-visible-log-indices', () => ({
  useVisibleLogIndexModel: () => ({
    mode: 'direct' as const,
    count: 0,
    version: 0,
    getLogicalIndex: (index: number) => index,
    indices: [],
  }),
}));

vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: () => ({
    getTotalSize: () => 0,
    getVirtualItems: () => [],
    scrollToIndex: vi.fn(),
  }),
}));

vi.mock('@/lib/log-api', () => ({
  fetchStructuredLogs: vi.fn().mockResolvedValue({ entries: [], total: 0 }),
}));

afterEach(() => {
  cleanup();
});

describe('logs panel shell constants (chrome + data tiers)', () => {
  it('defines a chrome-tier toolbar shell with panel surface and pointer-events-auto', () => {
    expect(logsToolbarShellClassName).toContain('bg-surface-1');
    expect(logsToolbarShellClassName).toContain('border-line');
    expect(logsToolbarShellClassName).toContain('pointer-events-auto');
    expect(logsToolbarShellClassName).not.toContain('backdrop-blur');
  });

  it('defines a data-tier table shell with panel surface and pointer-events-auto', () => {
    expect(logsTableShellClassName).toContain('bg-surface-1');
    expect(logsTableShellClassName).toContain('pointer-events-auto');
  });

  it('defines session list and archive panel data-tier shells', () => {
    expect(logsSessionListShellClassName).toContain('bg-surface-1');
    expect(logsArchivePanelShellClassName).toContain('pointer-events-auto');
  });

  it('defines sheet content panel without row animation classes', () => {
    expect(logsSheetContentClassName).toContain('bg-surface-1');
    expect(logsSheetContentClassName).not.toContain('backdrop-blur');
    expect(logsSheetContentClassName).not.toContain('animate-in');
  });
});

describe('LogsModeTabs panel chrome', () => {
  it('renders mode tabs inside a panel tabs list', () => {
    render(<LogsModeTabs mode="live" onModeChange={() => undefined} />);

    const liveTab = screen.getByRole('tab', { name: 'Live' });
    expect(liveTab.closest('[role="tablist"]')?.className).toContain('bg-surface-1');
    expect(screen.getByRole('tab', { name: 'Archive' })).toBeTruthy();
  });
});

describe('LogsToolbar panel chrome', () => {
  it('wraps filter controls in the toolbar shell with panel input', () => {
    render(
      <LogsFilterProvider>
        <LogsToolbar />
      </LogsFilterProvider>,
    );

    const shell = screen.getByTestId('logs-toolbar-shell');
    expect(shell.className).toContain('bg-surface-1');
    expect(screen.getByPlaceholderText('Filter message, source, level…')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Live' })).toBeTruthy();
  });
});

describe('LogsVirtualTable data-tier shell', () => {
  it('wraps the virtual log table in the data-tier shell', () => {
    render(
      <LogsFilterProvider>
        <LogsVirtualTable />
      </LogsFilterProvider>,
    );

    const shell = screen.getByTestId('logs-table-shell');
    expect(shell.className).toContain('bg-surface-1');
    expect(screen.getByText('No logs match the current filters.')).toBeTruthy();
  });
});

describe('LogsSessionList data-tier shell', () => {
  it('renders sessions inside the panel session list shell', () => {
    render(
      <LogsSessionList
        sessions={[
          {
            id: 'sess-1',
            label: 'bench-2026',
            started_ms: Date.now(),
            ended_ms: Date.now(),
            has_bench: true,
            has_candump: true,
            has_trace: false,
            candump_bytes: 1024,
            candump_frame_count: 42,
          },
        ]}
        selectedId="sess-1"
        onSelect={() => undefined}
      />,
    );

    const shell = screen.getByTestId('logs-session-list-shell');
    expect(shell.className).toContain('bg-surface-1');
    expect(screen.getByText('bench-2026')).toBeTruthy();
  });
});

describe('LogsArchiveSearch panel filters', () => {
  it('renders FTS input with panel variant and search action', () => {
    render(<LogsArchiveSearch />);

    const input = screen.getByPlaceholderText('joint, error, operator…');
    expect(input.className).toContain('bg-surface-1');
    expect(screen.getByRole('button', { name: 'Search' })).toBeTruthy();
  });
});

describe('LogDetailSheet panel sheet', () => {
  it('applies panel sheet content classes when open', () => {
    const entry: LogEntry = {
      id: 'log-1',
      timestamp: Date.now(),
      level: 'INFO',
      source: 'davout::motor',
      message: 'hold engaged',
    };

    render(<LogDetailSheet entry={entry} open onOpenChange={() => undefined} />);

    const content = document.querySelector('[data-slot="sheet-content"]');
    expect(content?.className).toContain('bg-surface-1');
    expect(screen.getByText('hold engaged')).toBeTruthy();
  });
});
