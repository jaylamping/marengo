// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { PresetCell } from '@/components/dashboard/inventory/cells/preset-cell';

afterEach(() => {
  cleanup();
});

describe('PresetCell', () => {
  it('is read-only and does not offer Assign preset', () => {
    render(
      <PresetCell itemId={20} preset="unassigned" jointName="right_elbow_pitch" />,
    );
    expect(screen.getByTestId('preset-cell-readonly')).toHaveTextContent('—');
    expect(screen.queryByRole('button', { name: /Assign preset/i })).toBeNull();
  });

  it('shows catalog tag text without an edit control', () => {
    render(
      <PresetCell itemId={1} preset="bench_default" jointName="pi5_can_hat" />,
    );
    expect(screen.getByTestId('preset-cell-readonly')).toHaveTextContent(
      'bench_default',
    );
    expect(screen.queryByRole('button')).toBeNull();
  });
});
