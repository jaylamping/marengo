import { useState } from 'react';

import { Button } from '@/components/ui/button';
import {
  SidebarIdentityRow,
  SidebarUpdateConfirmDialog,
  SidebarUpdateStatusView,
  statusCaption,
  type SidebarUpdateUiMode,
} from '@/components/dashboard/sidebar/sidebar-update-status-view';
import type { SidebarUser } from '@/data/sidebar-nav';

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

function previewUser(context: 'local' | 'live'): SidebarUser {
  return { name: 'Joey', context, avatar: '' };
}

/** DEV-only matrix of sidebar identity + deploy status states. */
export function SidebarUpdatePreviewPage() {
  const [confirmOpen, setConfirmOpen] = useState(false);

  return (
    <div className="min-h-screen bg-background px-6 py-8 text-foreground">
      <header className="mb-6 max-w-5xl">
        <h1 className="text-lg font-medium tracking-tight">Sidebar update states</h1>
        <p className="mt-1 font-mono text-xs text-muted-foreground">
          Production presentational components · DEV preview
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
                <SidebarIdentityRow
                  user={previewUser(frame.context)}
                  showUpdate={
                    (frame.mode === 'stale' || frame.mode === 'failed') && !frame.checking
                  }
                  onUpdate={() => setConfirmOpen(true)}
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
      <section className="mt-8 max-w-lg space-y-3">
        <p className="micro-label">Confirm dialog</p>
        <Button type="button" size="sm" variant="outline" onClick={() => setConfirmOpen(true)}>
          Open Update Marengo?
        </Button>
        <SidebarUpdateConfirmDialog
          open={confirmOpen}
          upstreamSha="b2c3d4e000000000000000000000000000000000"
          deploySha="a1b2c3d000000000000000000000000000000000"
          onOpenChange={setConfirmOpen}
          onCancel={() => setConfirmOpen(false)}
          onConfirm={() => setConfirmOpen(false)}
        />
      </section>
    </div>
  );
}
