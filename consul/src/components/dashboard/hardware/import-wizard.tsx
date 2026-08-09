import { useCallback, useState } from 'react';

import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { cn } from '@/lib/utils';
import {
  activateUrdf,
  fetchUrdfArchiveList,
  resolveUrdfPreview,
  restoreUrdfArchive,
  uploadUrdf,
  type FieldDiffDto,
  type FieldResolutionDto,
  type MergePreviewDto,
  type ResolutionChoice,
} from '@/lib/hardware-api';
import { queryClient } from '@/lib/query-client';
import { queryKeys } from '@/lib/query-keys';

type ImportWizardProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onActivated?: () => void;
};

type ResolutionMap = Record<string, ResolutionChoice>;

function resolutionKey(joint: string, field: string): string {
  return `${joint}::${field}`;
}

function criticalDiffs(preview: MergePreviewDto): FieldDiffDto[] {
  return preview.field_diffs.filter((d) => d.kinematics_critical);
}

export function ImportWizard({ open, onOpenChange, onActivated }: ImportWizardProps) {
  const [step, setStep] = useState<'pick' | 'resolve' | 'done'>('pick');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [uploadId, setUploadId] = useState<string | null>(null);
  const [preview, setPreview] = useState<MergePreviewDto | null>(null);
  const [resolutions, setResolutions] = useState<ResolutionMap>({});
  const [unresolved, setUnresolved] = useState<string[]>([]);
  const [activateMessage, setActivateMessage] = useState<string | null>(null);

  const reset = useCallback(() => {
    setStep('pick');
    setBusy(false);
    setError(null);
    setUploadId(null);
    setPreview(null);
    setResolutions({});
    setUnresolved([]);
    setActivateMessage(null);
  }, []);

  const handleOpenChange = (next: boolean) => {
    if (!next) {
      reset();
    }
    onOpenChange(next);
  };

  const onFilePick = async (file: File) => {
    setBusy(true);
    setError(null);
    try {
      const text = await file.text();
      const result = await uploadUrdf(text);
      if (!result?.ok) {
        setError('Upload failed — check gateway auth and URDF bytes.');
        return;
      }
      setUploadId(result.upload_id);
      setPreview(result.preview);
      const defaults: ResolutionMap = {};
      for (const diff of criticalDiffs(result.preview)) {
        defaults[resolutionKey(diff.joint, diff.field)] = 'master';
      }
      setResolutions(defaults);
      setStep('resolve');
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Upload failed');
    } finally {
      setBusy(false);
    }
  };

  const buildResolutionList = (): FieldResolutionDto[] =>
    Object.entries(resolutions).map(([key, choice]) => {
      const [joint, field] = key.split('::');
      return { joint, field, choice };
    });

  const runResolvePreview = async () => {
    if (!uploadId) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const result = await resolveUrdfPreview({
        upload_id: uploadId,
        resolutions: buildResolutionList(),
      });
      if (!result) {
        setError('Resolve preview failed.');
        return;
      }
      setPreview(result.preview);
      setUnresolved(result.unresolved_critical);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Resolve preview failed');
    } finally {
      setBusy(false);
    }
  };

  const runActivate = async () => {
    if (!uploadId) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const result = await activateUrdf({
        upload_id: uploadId,
        resolutions: buildResolutionList(),
      });
      if (!result?.ok) {
        setError(result?.message ?? 'Activate failed.');
        return;
      }
      setActivateMessage(result.message);
      setStep('done');
      await queryClient.invalidateQueries({ queryKey: queryKeys.configSnapshot });
      await queryClient.invalidateQueries({ queryKey: queryKeys.hardwareCompleteness });
      onActivated?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Activate failed');
    } finally {
      setBusy(false);
    }
  };

  const onRestoreArchive = async (id: string) => {
    setBusy(true);
    setError(null);
    try {
      const result = await restoreUrdfArchive(id);
      if (!result?.ok) {
        setError('Archive restore failed.');
        return;
      }
      setUploadId(result.upload_id);
      setPreview(result.preview);
      const defaults: ResolutionMap = {};
      for (const diff of criticalDiffs(result.preview)) {
        defaults[resolutionKey(diff.joint, diff.field)] = 'master';
      }
      setResolutions(defaults);
      setStep('resolve');
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Restore failed');
    } finally {
      setBusy(false);
    }
  };

  const critical = preview ? criticalDiffs(preview) : [];
  const canActivate =
    uploadId !== null &&
    critical.every((d) => resolutions[resolutionKey(d.joint, d.field)] != null);

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="max-w-lg" data-testid="import-wizard">
        <DialogHeader>
          <DialogTitle>Import URDF</DialogTitle>
          <DialogDescription>
            Upload contributor → resolve kinematics-critical fields → Accept activates master.
            Cancel leaves active URDF unchanged.
          </DialogDescription>
        </DialogHeader>

        {error ? (
          <p className="text-xs text-fault" role="status">{error}</p>
        ) : null}

        {step === 'pick' ? (
          <div className="flex flex-col gap-3">
            <label className="flex flex-col gap-2">
              <span className="micro-label">Contributor URDF file</span>
              <input
                type="file"
                accept=".urdf,.xml"
                disabled={busy}
                data-testid="import-file-input"
                onChange={(e) => {
                  const file = e.target.files?.[0];
                  if (file) {
                    void onFilePick(file);
                  }
                }}
              />
            </label>
            <ArchiveRestorePicker disabled={busy} onRestore={onRestoreArchive} />
          </div>
        ) : null}

        {step === 'resolve' && preview ? (
          <div className="flex flex-col gap-3 max-h-[50vh] overflow-y-auto">
            {critical.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                No kinematics-critical conflicts — ready to activate.
              </p>
            ) : (
              critical.map((diff) => {
                const key = resolutionKey(diff.joint, diff.field);
                const choice = resolutions[key] ?? 'master';
                return (
                  <div
                    key={key}
                    className="rounded-sm border border-line px-3 py-2 text-xs"
                    data-testid={`resolve-field-${diff.joint}-${diff.field}`}
                  >
                    <div className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
                      {diff.joint} · {diff.field}
                    </div>
                    <div className="mt-1 grid grid-cols-2 gap-2">
                      <button
                        type="button"
                        className={cn(
                          'rounded-sm border px-2 py-1 text-left',
                          choice === 'master' ? 'border-accent bg-accent/10' : 'border-line',
                        )}
                        onClick={() =>
                          setResolutions((prev) => ({ ...prev, [key]: 'master' }))
                        }
                      >
                        <span className="micro-label">master</span>
                        <div className="data-value">{diff.master_value}</div>
                      </button>
                      <button
                        type="button"
                        className={cn(
                          'rounded-sm border px-2 py-1 text-left',
                          choice === 'contributor' ? 'border-accent bg-accent/10' : 'border-line',
                        )}
                        onClick={() =>
                          setResolutions((prev) => ({ ...prev, [key]: 'contributor' }))
                        }
                      >
                        <span className="micro-label">contributor</span>
                        <div className="data-value">{diff.contributor_value}</div>
                      </button>
                    </div>
                  </div>
                );
              })
            )}
            {unresolved.length > 0 ? (
              <p className="text-xs text-accent">
                Unresolved critical: {unresolved.join(', ')}
              </p>
            ) : null}
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={busy}
              onClick={() => void runResolvePreview()}
            >
              Preview merge
            </Button>
          </div>
        ) : null}

        {step === 'done' ? (
          <p className="text-sm text-ok" role="status">{activateMessage}</p>
        ) : null}

        <DialogFooter className="gap-2 sm:gap-0">
          <Button
            type="button"
            variant="outline"
            disabled={busy}
            onClick={() => handleOpenChange(false)}
          >
            {step === 'done' ? 'Close' : 'Cancel'}
          </Button>
          {step === 'resolve' ? (
            <Button
              type="button"
              disabled={busy || !canActivate}
              data-testid="import-accept"
              onClick={() => void runActivate()}
            >
              {busy ? 'Activating…' : 'Accept → Active'}
            </Button>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ArchiveRestorePicker({
  disabled,
  onRestore,
}: {
  disabled: boolean;
  onRestore: (uploadId: string) => void;
}) {
  const [ids, setIds] = useState<string[]>([]);
  const [loaded, setLoaded] = useState(false);

  if (!loaded) {
    void fetchUrdfArchiveList().then((list) => {
      setIds(list?.entries.map((e) => e.upload_id) ?? []);
      setLoaded(true);
    });
  }

  if (ids.length === 0) {
    return null;
  }

  return (
    <div className="flex flex-col gap-1">
      <span className="micro-label">Or restore archive</span>
      <div className="flex flex-wrap gap-2">
        {ids.slice(0, 5).map((id) => (
          <Button
            key={id}
            type="button"
            size="sm"
            variant="outline"
            disabled={disabled}
            onClick={() => onRestore(id)}
          >
            {id}
          </Button>
        ))}
      </div>
    </div>
  );
}
