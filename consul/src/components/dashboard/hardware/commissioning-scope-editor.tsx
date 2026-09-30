import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';

import { Button } from '@/components/ui/button';
import { useConfigSnapshot } from '@/hooks/use-config-snapshot';
import {
  type CommissioningScopeResponse,
  scopeWidens,
} from '@/lib/commissioning-scope';
import {
  deleteCommissioningScope,
  fetchCommissioningScope,
  putCommissioningScope,
} from '@/lib/gateway-api';
import { queryKeys } from '@/lib/query-keys';
import { cn } from '@/lib/utils';

function parseJointList(raw: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const part of raw.split(/[\n,\s]+/)) {
    const name = part.trim();
    if (!name || seen.has(name)) {
      continue;
    }
    seen.add(name);
    out.push(name);
  }
  return out;
}

function formatJoints(joints: string[]): string {
  return joints.join('\n');
}

export function CommissioningScopeEditor({ className }: { className?: string }) {
  const queryClient = useQueryClient();
  const { data: snapshot } = useConfigSnapshot();
  const masterJoints = snapshot?.joints ?? [];

  const scopeQuery = useQuery({
    queryKey: queryKeys.commissioningScope,
    queryFn: fetchCommissioningScope,
    staleTime: 15_000,
  });

  const [draftText, setDraftText] = useState<string | null>(null);
  const [widenPending, setWidenPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const scope: CommissioningScopeResponse | undefined = scopeQuery.data;
  const draft =
    draftText ??
    formatJoints(scope?.joints ?? scope?.effective ?? []);

  const draftJoints = useMemo(() => parseJointList(draft), [draft]);

  const putMutation = useMutation({
    mutationFn: putCommissioningScope,
    onSuccess: async (next) => {
      setWidenPending(false);
      setError(null);
      setDraftText(formatJoints(next.joints));
      await queryClient.invalidateQueries({ queryKey: queryKeys.commissioningScope });
    },
    onError: (err: unknown) => {
      setError(err instanceof Error ? err.message : 'Scope apply failed');
    },
  });

  const clearMutation = useMutation({
    mutationFn: deleteCommissioningScope,
    onSuccess: async (next) => {
      setWidenPending(false);
      setError(null);
      setDraftText(formatJoints(next.joints));
      await queryClient.invalidateQueries({ queryKey: queryKeys.commissioningScope });
    },
    onError: (err: unknown) => {
      setError(err instanceof Error ? err.message : 'Scope clear failed');
    },
  });

  const apply = (confirmWiden: boolean) => {
    setError(null);
    const previous = scope?.effective ?? [];
    const nextEffectiveGuess = draftJoints;
    if (!confirmWiden && scopeWidens(previous, nextEffectiveGuess)) {
      setWidenPending(true);
      return;
    }
    putMutation.mutate({
      joints: draftJoints,
      confirm_widen: confirmWiden,
    });
  };

  return (
    <section
      className={cn(
        'flex flex-col gap-3 rounded-sm border border-line bg-surface-1 px-3 py-3',
        className,
      )}
      data-testid="commissioning-scope-editor"
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div>
          <h2 className="font-sans text-sm tracking-tight text-foreground">
            Commissioning scope
          </h2>
          <p className="micro-label">
            Persist Enable targets · effective = persisted ∩ startup ceiling
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            size="sm"
            variant="secondary"
            data-testid="scope-apply-btn"
            disabled={putMutation.isPending || clearMutation.isPending}
            onClick={() => apply(false)}
          >
            Apply scope
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            data-testid="scope-clear-btn"
            disabled={
              putMutation.isPending ||
              clearMutation.isPending ||
              scope?.persisted === false
            }
            onClick={() => clearMutation.mutate()}
          >
            Clear scope
          </Button>
        </div>
      </div>

      <div className="grid gap-3 md:grid-cols-2">
        <label className="flex flex-col gap-1">
          <span className="micro-label">Joints (one per line)</span>
          <textarea
            className="min-h-28 rounded-md border border-line bg-surface-0 px-2 py-1.5 font-mono text-xs text-foreground"
            data-testid="scope-joints-input"
            value={draft}
            onChange={(e) => {
              setDraftText(e.target.value);
              setWidenPending(false);
            }}
            spellCheck={false}
          />
        </label>
        <div className="flex flex-col gap-2 text-xs">
          <div>
            <span className="micro-label">Effective</span>
            <p className="font-mono text-foreground" data-testid="scope-effective">
              {(scope?.effective ?? []).length > 0
                ? scope!.effective.join(', ')
                : 'none (no-scope → Robot Ready gate)'}
            </p>
          </div>
          <div>
            <span className="micro-label">Ceiling</span>
            <p className="font-mono text-muted-foreground">
              {scope?.ceiling && scope.ceiling.length > 0
                ? scope.ceiling.join(', ')
                : 'none'}
            </p>
          </div>
          {masterJoints.length > 0 ? (
            <div>
              <span className="micro-label">Master joints</span>
              <p className="font-mono text-muted-foreground">
                {masterJoints.join(', ')}
              </p>
            </div>
          ) : null}
        </div>
      </div>

      {widenPending ? (
        <div
          className="flex flex-wrap items-center gap-2 rounded-sm border border-amber-500/40 bg-amber-500/10 px-2 py-2"
          data-testid="scope-widen-confirm"
          role="alertdialog"
        >
          <p className="text-xs text-foreground">
            This widens energizable scope. Confirm before applying.
          </p>
          <Button
            type="button"
            size="sm"
            data-testid="scope-widen-confirm-btn"
            disabled={putMutation.isPending}
            onClick={() => apply(true)}
          >
            Confirm widen
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => setWidenPending(false)}
          >
            Cancel
          </Button>
        </div>
      ) : null}

      {error ? (
        <p className="text-xs text-destructive" role="alert">
          {error}
        </p>
      ) : null}
      {scopeQuery.isError ? (
        <p className="text-xs text-destructive" role="alert">
          {scopeQuery.error instanceof Error
            ? scopeQuery.error.message
            : 'Failed to load commissioning scope'}
        </p>
      ) : null}
    </section>
  );
}
