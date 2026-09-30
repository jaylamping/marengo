// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { MemoryRouter } from 'react-router-dom';

import { InventoryLimitsReadOnly } from '@/components/dashboard/hardware/hardware-settings-sheet';

afterEach(() => {
  cleanup();
});

describe('InventoryLimitsReadOnly', () => {
  it('shows read-only range and deep-link to Hardware', () => {
    render(
      <MemoryRouter>
        <InventoryLimitsReadOnly
          jointName="right_shoulder_pitch"
          liveRange="−0.90–2.93"
        />
      </MemoryRouter>,
    );
    expect(screen.getByTestId('inventory-limits-readonly')).toBeTruthy();
    expect(screen.getByText(/Range \(live\): −0.90–2.93/)).toBeTruthy();
    const link = screen.getByRole('link', { name: 'Calibrate on Hardware' });
    expect(link.getAttribute('href')).toBe('/hardware');
    expect(screen.queryByRole('button', { name: 'Set Limits' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Apply Limits' })).toBeNull();
  });
});
