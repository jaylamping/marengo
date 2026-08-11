import { Loading03Icon, RefreshIcon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';

import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

export type SidebarUpdateUiMode =
  | 'unknown'
  | 'current'
  | 'stale'
  | 'upstream_unknown'
  | 'updating'
  | 'failed';

export type SidebarUpdateStatusViewProps = {
  mode: SidebarUpdateUiMode;
  caption: string;
  shaTitle?: string;
  checking?: boolean;
  error?: string | null;
  onCheck?: () => void;
  className?: string;
};

export function statusLedClass(mode: SidebarUpdateUiMode): string {
  switch (mode) {
    case 'current':
      return 'led led-ok';
    case 'stale':
      return 'led led-info';
    case 'failed':
      return 'led led-fault';
    case 'updating':
      return 'led led-info led-live';
    case 'upstream_unknown':
    case 'unknown':
      return 'led';
    default: {
      const _exhaustive: never = mode;
      return _exhaustive;
    }
  }
}

export function phaseLabel(phase: string | undefined): string | null {
  if (!phase) return null;
  const map: Record<string, string> = {
    init: 'Init',
    dirty: 'Dirty tree',
    fetch: 'Fetch',
    lfs: 'LFS',
    build: 'Build',
    install: 'Install',
    enqueue: 'Queued',
    done: 'Done',
    timeout: 'Timed out',
    orphan: 'Interrupted',
    error: 'Error',
  };
  return map[phase] ?? phase;
}

export function statusCaption(
  mode: SidebarUpdateUiMode,
  shaLabel: string,
  phase: string | null,
): string {
  switch (mode) {
    case 'updating':
      return phase ? `Updating · ${phase}` : 'Updating…';
    case 'stale':
      return `rev ${shaLabel} · behind`;
    case 'upstream_unknown':
      return `rev ${shaLabel} · offline`;
    case 'failed':
      return `rev ${shaLabel} · failed`;
    case 'current':
      return `rev ${shaLabel}`;
    case 'unknown':
      return 'rev —';
    default: {
      const _exhaustive: never = mode;
      return _exhaustive;
    }
  }
}

/** Info chip — lives on the identity row next to Joey / live. */
export function SidebarUpdateButton({
  onClick,
  className,
}: {
  onClick?: () => void;
  className?: string;
}) {
  return (
    <button
      type="button"
      data-testid="sidebar-update-button"
      className={cn(
        'inline-flex h-6 shrink-0 items-center border border-info/50 bg-info/10 px-2 font-mono text-[10px] uppercase tracking-[0.14em] text-info transition-colors',
        'hover:bg-info/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
        className,
      )}
      title="Install latest main onto the Pi"
      onClick={onClick}
    >
      Update
    </button>
  );
}

/** Presentational deploy status chrome (sidebar footer). Check sits on the rev row. */
export function SidebarUpdateStatusView({
  mode,
  caption,
  shaTitle,
  checking = false,
  error = null,
  onCheck,
  className,
}: SidebarUpdateStatusViewProps) {
  const updating = mode === 'updating';

  return (
    <div
      className={cn(
        'flex w-full flex-col gap-1 border-t border-line px-2 pt-2',
        className,
      )}
      data-testid="sidebar-update-status"
    >
      <div className="flex min-w-0 items-center gap-1.5">
        <span className={statusLedClass(mode)} aria-hidden />
        <span
          className="micro-label min-w-0 flex-1 truncate normal-case tracking-normal"
          title={shaTitle}
        >
          {caption}
        </span>
        {updating ? (
          <HugeiconsIcon
            icon={Loading03Icon}
            strokeWidth={2}
            className="size-3.5 shrink-0 animate-spin text-info motion-reduce:animate-none"
            data-testid="sidebar-update-spinner"
            aria-hidden
          />
        ) : null}
        <Button
          type="button"
          variant="ghost"
          size="xs"
          className="ml-auto h-6 shrink-0 px-1.5 text-muted-foreground hover:text-foreground"
          disabled={updating || checking}
          data-testid="check-for-updates"
          aria-label="Check for updates"
          title="Check for updates"
          onClick={onCheck}
        >
          {checking ? (
            <HugeiconsIcon
              icon={Loading03Icon}
              strokeWidth={2}
              className="size-3 animate-spin motion-reduce:animate-none"
              data-icon="inline-start"
            />
          ) : (
            <HugeiconsIcon
              icon={RefreshIcon}
              strokeWidth={2}
              className="size-3"
              data-icon="inline-start"
            />
          )}
          Check
        </Button>
      </div>

      {error ? (
        <p className="text-[10px] leading-snug text-fault" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
