import {
  Avatar,
  AvatarFallback,
} from '@/components/ui/avatar';
import {
  SidebarUpdateButton,
  SidebarUpdateStatusView,
  statusCaption,
  type SidebarUpdateUiMode,
} from '@/components/dashboard/sidebar/sidebar-update-status-view';

type Frame = {
  title: string;
  context: 'local' | 'live';
  mode: SidebarUpdateUiMode;
  sha: string;
  phase?: string | null;
  checking?: boolean;
  error?: string | null;
};

const frames: Frame[] = [
  {
    title: 'Dev · current',
    context: 'local',
    mode: 'current',
    sha: 'a1b2c3d',
  },
  {
    title: 'Live · current',
    context: 'live',
    mode: 'current',
    sha: 'a1b2c3d',
  },
  {
    title: 'Live · behind (Update)',
    context: 'live',
    mode: 'stale',
    sha: 'a1b2c3d',
  },
  {
    title: 'Check in flight',
    context: 'live',
    mode: 'current',
    sha: 'a1b2c3d',
    checking: true,
  },
  {
    title: 'Updating · Build',
    context: 'live',
    mode: 'updating',
    sha: 'a1b2c3d',
    phase: 'Build',
  },
  {
    title: 'Updating · Install',
    context: 'live',
    mode: 'updating',
    sha: 'a1b2c3d',
    phase: 'Install',
  },
  {
    title: 'GitHub offline',
    context: 'live',
    mode: 'upstream_unknown',
    sha: 'a1b2c3d',
  },
  {
    title: 'Failed',
    context: 'live',
    mode: 'failed',
    sha: 'a1b2c3d',
    error: 'dirty working tree — commit or stash first',
  },
  {
    title: 'Unknown / offline gateway',
    context: 'local',
    mode: 'unknown',
    sha: '—',
  },
];

function Identity({
  context,
  showUpdate,
}: {
  context: 'local' | 'live';
  showUpdate?: boolean;
}) {
  return (
    <div className="flex w-full items-center gap-2 rounded-md px-2 py-1.5">
      <Avatar className="size-8 rounded-lg grayscale">
        <AvatarFallback className="rounded-lg">J</AvatarFallback>
      </Avatar>
      <div className="grid min-w-0 flex-1 text-left text-sm leading-tight">
        <span className="truncate font-medium">Joey</span>
        <span className="truncate font-mono text-xs text-muted-foreground">
          {context}
        </span>
      </div>
      {showUpdate ? <SidebarUpdateButton /> : null}
    </div>
  );
}

/** DEV-only matrix of sidebar identity + deploy status states. */
export function SidebarUpdatePreviewPage() {
  return (
    <div className="min-h-screen bg-background px-6 py-8 text-foreground">
      <header className="mb-6 max-w-5xl">
        <h1 className="text-lg font-medium tracking-tight">Sidebar update states</h1>
        <p className="mt-1 font-mono text-xs text-muted-foreground">
          Identity context + deploy chrome · DEV preview
        </p>
      </header>
      <div className="grid max-w-5xl gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {frames.map((frame) => {
          const caption = statusCaption(
            frame.mode,
            frame.sha,
            frame.phase ?? null,
          );
          return (
            <section
              key={frame.title}
              className="overflow-hidden rounded-md border border-line bg-surface-1"
              data-testid={`preview-${frame.mode}-${frame.context}`}
            >
              <div className="border-b border-line px-3 py-2">
                <p className="micro-label">{frame.title}</p>
              </div>
              <div className="w-[16.5rem] bg-sidebar px-0 py-1">
                <Identity
                  context={frame.context}
                  showUpdate={frame.mode === 'stale' && !frame.checking}
                />
                <SidebarUpdateStatusView
                  mode={frame.mode}
                  caption={caption}
                  shaTitle={frame.sha === '—' ? undefined : `${frame.sha}${'0'.repeat(33)}`}
                  checking={frame.checking}
                  error={frame.error}
                />
              </div>
            </section>
          );
        })}
      </div>
      <section className="mt-8 max-w-lg rounded-md border border-line bg-surface-1 p-4">
        <p className="micro-label mb-3">Confirm dialog copy</p>
        <h2 className="text-base font-medium">Update Marengo?</h2>
        <p className="mt-2 text-sm text-muted-foreground">
          Pins GitHub main on the Pi, builds natively, and installs to /opt/marengo.
          Several minutes of downtime. Support elevated arms — motors go limp during
          install.
        </p>
        <p className="mt-3 font-mono text-xs text-muted-foreground">
          <span className="text-info">b2c3d4e</span>
          <span className="text-faint"> ← </span>
          a1b2c3d
        </p>
        <div className="mt-4 flex justify-end gap-2">
          <span className="inline-flex h-8 items-center rounded-md border border-border px-3 text-xs">
            Cancel
          </span>
          <span className="inline-flex h-8 items-center rounded-md border border-info/40 bg-info/90 px-3 text-xs text-background">
            Update now
          </span>
        </div>
      </section>
    </div>
  );
}
