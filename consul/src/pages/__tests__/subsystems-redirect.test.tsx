// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';

import { SubsystemsPage } from '@/pages/subsystems';

afterEach(() => {
  cleanup();
});

function LocationProbe() {
  const location = useLocation();
  return <div data-testid="location">{location.pathname}</div>;
}

describe('SubsystemsPage redirect', () => {
  it('redirects /subsystems to /telemetry', () => {
    render(
      <MemoryRouter initialEntries={['/subsystems']}>
        <Routes>
          <Route path="/subsystems" element={<SubsystemsPage />} />
          <Route
            path="/telemetry"
            element={
              <>
                <LocationProbe />
                <div>Telemetry stub</div>
              </>
            }
          />
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getByTestId('location').textContent).toBe('/telemetry');
    expect(screen.getByText('Telemetry stub')).toBeTruthy();
  });

  it('does not render Inventory commissioning chrome at /subsystems', () => {
    render(
      <MemoryRouter initialEntries={['/subsystems']}>
        <Routes>
          <Route path="/subsystems" element={<SubsystemsPage />} />
          <Route path="/telemetry" element={<div data-testid="telemetry-landed" />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.queryByText(/devices · actuators · sensors/i)).toBeNull();
    expect(screen.getByTestId('telemetry-landed')).toBeTruthy();
  });
});
