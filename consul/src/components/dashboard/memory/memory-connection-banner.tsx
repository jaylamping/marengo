import { MEM0_DASHBOARD_URL, isMem0Live } from '@/lib/mem0-config';
import { cn } from '@/lib/utils';

type MemoryConnectionBannerProps = {
  reachable: boolean | null;
};

export function MemoryConnectionBanner({ reachable }: MemoryConnectionBannerProps) {
  const mode =
    !isMem0Live()
      ? 'unconfigured'
      : reachable === null
        ? 'loading'
        : reachable
          ? 'connected'
          : 'offline';

  return (
    <div className="flex flex-wrap items-center gap-3 rounded-lg border bg-muted/30 px-3 py-2 text-sm">
      <span
        className={cn(
          'inline-flex items-center gap-1.5 font-medium',
          mode === 'connected' && 'text-emerald-600',
          mode === 'loading' && 'text-amber-600',
          (mode === 'offline' || mode === 'unconfigured') && 'text-muted-foreground',
        )}
      >
        <span
          className={cn(
            'size-2 rounded-full',
            mode === 'connected' && 'bg-emerald-500',
            mode === 'loading' && 'bg-amber-400 animate-pulse',
            (mode === 'offline' || mode === 'unconfigured') && 'bg-muted-foreground',
          )}
        />
        mem0 {mode}
      </span>
      {mode === 'unconfigured' ? (
        <span className="text-muted-foreground">
          Set MEM0_API_URL + MEM0_API_KEY in consul/.env.local (see .env.example)
        </span>
      ) : null}
      {mode === 'offline' ? (
        <span className="text-destructive">Cannot reach mem0 via Vite proxy — check Tailscale + docker on joey-pc</span>
      ) : null}
      <a
        href={MEM0_DASHBOARD_URL}
        target="_blank"
        rel="noreferrer"
        className="text-primary underline-offset-4 hover:underline"
      >
        mem0 dashboard
      </a>
    </div>
  );
}
