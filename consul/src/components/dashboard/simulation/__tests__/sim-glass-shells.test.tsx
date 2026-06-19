// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { cardVariants } from '@/components/ui/card';
import {
  simControlBarClassName,
  simDataShellVariant,
  simOverviewShellClassName,
} from '@/components/dashboard/simulation/constants';
import { SimControlBar } from '@/components/dashboard/simulation/sim-control-bar';
import { SimEventLog } from '@/components/dashboard/simulation/sim-event-log';
import { SimRuntimeMetricsCard } from '@/components/dashboard/simulation/sim-runtime-metrics-card';
import { SimScenariosTable } from '@/components/dashboard/simulation/sim-scenarios-table';
import { SimulationOverview } from '@/components/dashboard/simulation/simulation-overview';
import { SimSessionCard } from '@/components/dashboard/simulation/sim-session-card';
import { SimViewportPlaceholder } from '@/components/dashboard/simulation/sim-viewport-placeholder';
import {
  dummySimEvents,
  dummySimRuntimeMetrics,
  dummySimScenarios,
  dummySimSession,
} from '@/data/simulation';

vi.mock('@hugeicons/react', () => ({
  HugeiconsIcon: () => <span data-testid="hugeicon" />,
}));

afterEach(() => {
  cleanup();
});

function expectGlassCard(element: Element | null) {
  expect(element).toBeTruthy();
  expect(element?.className).toMatch(/backdrop-blur-xl/);
  expect(element?.className).toContain('[border-top-color:var(--glass-refraction-top)]');
}

describe('Simulation glass constants', () => {
  it('defines chrome-tier control bar with blur, refraction, and pointer events', () => {
    expect(simControlBarClassName).toContain('backdrop-blur-xl');
    expect(simControlBarClassName).toContain('[border-top-color:var(--glass-refraction-top)]');
    expect(simControlBarClassName).toContain('pointer-events-auto');
  });

  it('maps data shells to the GLINUI glass card variant', () => {
    const classes = cardVariants({ variant: simDataShellVariant });
    expect(classes).toContain('backdrop-blur-xl');
  });

  it('reuses dashboard panel pointer threading for the route shell', () => {
    expect(simOverviewShellClassName).toContain('pointer-events-auto');
  });
});

describe('SimControlBar (chrome tier)', () => {
  it('renders Play with glass styling when session is idle', () => {
    render(<SimControlBar sessionState="idle" />);

    const play = screen.getByRole('button', { name: /play/i });
    expect(play.className).toMatch(/backdrop-blur-xl/);
    expect(screen.getByTestId('sim-control-bar').className).toContain('backdrop-blur-xl');
  });

  it('disables transport controls when disconnected', () => {
    render(<SimControlBar sessionState="disconnected" />);

    for (const button of screen.getAllByRole('button')) {
      expect((button as HTMLButtonElement).disabled).toBe(true);
    }
  });
});

describe('SimScenariosTable (data tier)', () => {
  it('renders scenarios inside a glass card shell', () => {
    render(<SimScenariosTable scenarios={dummySimScenarios} />);

    expectGlassCard(document.querySelector('[data-slot="card"]'));
    expect(screen.getByText('bench_home')).toBeTruthy();
    expect(screen.getByText('pick_mug_smoke')).toBeTruthy();
  });

  it('keeps table rows on an opaque background inside the glass shell', () => {
    render(<SimScenariosTable scenarios={dummySimScenarios.slice(0, 1)} />);

    const row = document.querySelector('[data-slot="table-body"] [data-slot="table-row"]');
    expect(row?.className).toContain('bg-card/95');
  });
});

describe('SimViewportPlaceholder (hero tier)', () => {
  it('renders the viewport hero card with glass styling', () => {
    render(<SimViewportPlaceholder />);

    expectGlassCard(document.querySelector('[data-slot="card"]'));
    expect(screen.getByText('Isaac Sim stage')).toBeTruthy();
    expect(screen.getByText(/Live viewport stream/i)).toBeTruthy();
  });
});

describe('SimEventLog (data tier)', () => {
  it('renders events inside a glass card with an opaque log well', () => {
    render(<SimEventLog events={dummySimEvents} />);

    expectGlassCard(document.querySelector('[data-slot="card"]'));
    const well = screen.getByTestId('sim-event-log-well');
    expect(well.className).toContain('bg-card/95');
    expect(screen.getByText(/Wireframe session/i)).toBeTruthy();
  });
});

describe('Simulation metric cards (data tier)', () => {
  it('renders session card with glass shell', () => {
    render(<SimSessionCard session={dummySimSession} />);

    expectGlassCard(document.querySelector('[data-slot="card"]'));
    expect(screen.getByText('Isaac Sim 4.5.0')).toBeTruthy();
  });

  it('renders runtime metrics card with glass shell', () => {
    render(<SimRuntimeMetricsCard metrics={dummySimRuntimeMetrics} />);

    expectGlassCard(document.querySelector('[data-slot="card"]'));
    expect(screen.getByText('Physics + render')).toBeTruthy();
  });
});

describe('SimulationOverview', () => {
  it('opts the route shell back into pointer events for glass panels', () => {
    render(<SimulationOverview />);

    expect(screen.getByTestId('simulation-overview').className).toContain(
      'pointer-events-auto',
    );
    expect(screen.getByText('Isaac Lab tasks')).toBeTruthy();
  });
});
