import { describe, expect, it } from 'vitest';

import { deriveSidebarUpdateMode } from '@/components/dashboard/sidebar/use-sidebar-self-update';
import type { VersionStatusDto } from '@/lib/version-api';

function status(
  partial: Partial<VersionStatusDto> & {
    deploy?: Partial<VersionStatusDto['deploy']>;
  } = {},
): VersionStatusDto {
  const { deploy: deployPartial, ...rest } = partial;
  return {
    deploy_sha: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
    upstream_sha: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
    upstream_ok: true,
    update_available: true,
    ready_for_target: false,
    ui_state: 'stale',
    deploy: {
      state: 'idle',
      job_id: 'j1',
      target_sha: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
      result_sha: '',
      unit_name: 'marengo-self-update',
      started_at: '2026-08-11T00:00:00Z',
      updated_at: '2026-08-11T00:00:00Z',
      message: '',
      phase: 'init',
      ...deployPartial,
    },
    ...rest,
  };
}

describe('deriveSidebarUpdateMode', () => {
  it('keeps updating across reload while a watched job has no status yet', () => {
    expect(
      deriveSidebarUpdateMode(null, { watchingJob: true }),
    ).toBe('updating');
  });

  it('does not flash behind when watched job is still running but ui_state lags', () => {
    expect(
      deriveSidebarUpdateMode(
        status({
          ui_state: 'stale',
          deploy: { state: 'running', phase: 'build' },
        }),
        { watchingJob: true },
      ),
    ).toBe('updating');
  });

  it('shows failed for a watched job instead of remapping to behind', () => {
    expect(
      deriveSidebarUpdateMode(
        status({
          ui_state: 'stale',
          update_available: true,
          deploy: { state: 'failed', phase: 'error', message: 'boom' },
        }),
        { watchingJob: true },
      ),
    ).toBe('failed');
  });

  it('falls through to stale when not watching a job', () => {
    expect(
      deriveSidebarUpdateMode(
        status({ ui_state: 'stale' }),
        { watchingJob: false },
      ),
    ).toBe('stale');
  });
});
