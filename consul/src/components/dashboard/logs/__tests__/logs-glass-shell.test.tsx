// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

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
import { fetchStructuredLogs } from '@/lib/log-api';

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

vi.mock('@/lib/log-api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/lib/log-api')>();
  return {
    ...actual,
    fetchStructuredLogs: vi.fn().mockResolvedValue({ ok: true, data: { entries: [], total: 0 } }),
  };
});

const mockFetchStructuredLogs = vi.mocked(fetchStructuredLogs);

afterEach(() => {
  cleanup();
  mockFetchStructuredLogs.mockResolvedValue({ ok: true, data: { entries: [], total: 0 } });
});

describe('logs glass shell constants (chrome + data tiers)', () => {
  it('defines a chrome-tier toolbar shell with blur and pointer-events-auto', () => {
    expect(logsToolbarShellClassName).toContain('backdrop-blur-xl');
    expect(logsToolbarShellClassName).toContain('pointer-events-auto');
    expect(logsToolbarShellClassName).toContain(
      '[border-top-color:var(--glass-refraction-top)]',
    );
  });

  it('defines a data-tier table shell with blur and pointer-events-auto', () => {
    expect(logsTableShellClassName).toContain('backdrop-blur-xl');
    expect(logsTableShellClassName).toContain('pointer-events-auto');
  });

  it('defines session list and archive panel data-tier shells', () => {
    expect(logsSessionListShellClassName).toContain('backdrop-blur-xl');
    expect(logsArchivePanelShellClassName).toContain('pointer-events-auto');
  });

  it('defines sheet content glass without row animation classes', () => {
    expect(logsSheetContentClassName).toContain('backdrop-blur-xl');
    expect(logsSheetContentClassName).not.toContain('animate-in');
  });
});

describe('LogsModeTabs glass chrome', () => {
  it('renders mode tabs inside a glass tabs list', () => {
    render(<LogsModeTabs mode="live" onModeChange={() => undefined} />);

    const liveTab = screen.getByRole('tab', { name: 'Live' });
    expect(liveTab.closest('[role="tablist"]')?.className).toMatch(/backdrop-blur-md/);
    expect(screen.getByRole('tab', { name: 'Archive' })).toBeTruthy();
  });
});

describe('LogsToolbar glass chrome', () => {
  it('wraps filter controls in the toolbar shell with glass input', () => {
    render(
      <LogsFilterProvider>
        <LogsToolbar />
      </LogsFilterProvider>,
    );

    const shell = screen.getByTestId('logs-toolbar-shell');
    expect(shell.className).toMatch(/backdrop-blur-xl/);
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
    expect(shell.className).toMatch(/backdrop-blur-xl/);
    expect(screen.getByText('No logs match the current filters.')).toBeTruthy();
  });
});

describe('LogsSessionList data-tier shell', () => {
  it('renders sessions inside the glass session list shell', () => {
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
    expect(shell.className).toMatch(/backdrop-blur-xl/);
    expect(screen.getByText('bench-2026')).toBeTruthy();
  });

  it('shows error banner without empty copy on server failure', () => {
    render(
      <LogsSessionList
        sessions={[]}
        selectedId={null}
        onSelect={() => undefined}
        error={{ kind: 'server', status: 500 }}
      />,
    );

    expect(screen.getByText('Log gateway returned a server error.')).toBeTruthy();
    expect(screen.queryByText('No archived sessions.')).toBeNull();
  });

  it('shows empty copy without banner on successful empty sessions', () => {
    render(
      <LogsSessionList sessions={[]} selectedId={null} onSelect={() => undefined} error={null} />,
    );

    expect(screen.getByText('No archived sessions.')).toBeTruthy();
    expect(screen.queryByText('Log gateway returned a server error.')).toBeNull();
  });
});

describe('LogsArchiveSearch glass filters', () => {
  it('renders FTS input with glass variant and search action', () => {
    render(<LogsArchiveSearch />);

    const input = screen.getByPlaceholderText('joint, error, operator…');
    expect(input.className).toMatch(/backdrop-blur/);
    expect(screen.getByRole('button', { name: 'Search' })).toBeTruthy();
  });

  it('surfaces HTTP failure instead of SQLite-empty messaging', async () => {
    mockFetchStructuredLogs.mockResolvedValueOnce({
      ok: false,
      error: { kind: 'unavailable', status: 503 },
    });

    render(<LogsArchiveSearch />);
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    expect(await screen.findByText('Log store temporarily unavailable.')).toBeTruthy();
    expect(screen.queryByText('No structured logs in SQLite yet.')).toBeNull();
  });

  it('distinguishes HTTP failure from ok-empty LogApiResult', async () => {
    mockFetchStructuredLogs.mockResolvedValueOnce({
      ok: false,
      error: { kind: 'server', status: 500 },
    });

    render(<LogsArchiveSearch />);
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    expect(await screen.findByText('Log gateway returned a server error.')).toBeTruthy();
    expect(screen.queryByText('No structured logs in SQLite yet.')).toBeNull();
  });
});

describe('LogDetailSheet glass sheet', () => {
  it('applies glass sheet content classes when open', () => {
    const entry: LogEntry = {
      id: 'log-1',
      timestamp: Date.now(),
      level: 'INFO',
      source: 'davout::motor',
      message: 'hold engaged',
    };

    render(<LogDetailSheet entry={entry} open onOpenChange={() => undefined} />);

    const content = document.querySelector('[data-slot="sheet-content"]');
    expect(content?.className).toMatch(/backdrop-blur-xl/);
    expect(screen.getByText('hold engaged')).toBeTruthy();
  });
});
