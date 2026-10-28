// @vitest-environment jsdom
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { ImportWizard } from '@/components/dashboard/hardware/import-wizard';
import { fetchUrdfArchiveList } from '@/lib/hardware-api';

vi.mock('@/lib/hardware-api', () => ({
  uploadUrdf: vi.fn(),
  resolveUrdfPreview: vi.fn(),
  activateUrdf: vi.fn(),
  // Never resolves: every render before the list arrives would refetch.
  fetchUrdfArchiveList: vi.fn(() => new Promise(() => {})),
  restoreUrdfArchive: vi.fn(),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('ImportWizard archive list (UI-29)', () => {
  it('fetches the archive list once, not on every render', () => {
    const props = { open: true, onOpenChange: () => undefined, onActivated: () => undefined };
    const { rerender } = render(<ImportWizard {...props} />);
    for (let i = 0; i < 3; i += 1) {
      rerender(<ImportWizard {...props} />);
    }
    expect(fetchUrdfArchiveList).toHaveBeenCalledTimes(1);
  });
});
