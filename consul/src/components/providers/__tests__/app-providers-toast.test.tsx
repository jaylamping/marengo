// @vitest-environment jsdom
import { describe, it, expect, vi, beforeAll } from 'vitest';
import { render, screen } from '@testing-library/react';

import { AppProviders } from '@/components/providers/app-providers';
import { Toaster, toasterPanelStyle } from '@/components/ui/sonner';

beforeAll(() => {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
});

vi.mock('@/hooks/use-chappe-telemetry', () => ({
  useChappeTelemetry: () => undefined,
}));

vi.mock('sonner', () => ({
  Toaster: ({
    theme,
    style,
    className,
  }: {
    theme?: string;
    style?: React.CSSProperties;
    className?: string;
  }) => (
    <div
      data-sonner-toaster
      data-theme={theme}
      className={className}
      style={style}
    />
  ),
}));

describe('AppProviders toast alignment (single GLINUI toaster)', () => {
  it('renders exactly one Toaster in the provider tree', () => {
    render(
      <AppProviders>
        <div>child</div>
      </AppProviders>,
    );

    expect(screen.getByText('child')).toBeTruthy();
    expect(document.querySelectorAll('[data-sonner-toaster]').length).toBe(1);
  });
});

describe('Toaster panel styling', () => {
  it('uses cn-toast class and panel CSS variables', () => {
    expect((toasterPanelStyle as Record<string, string>)['--normal-bg']).toBe(
      'var(--surface-2)',
    );
    expect((toasterPanelStyle as Record<string, string>)['--normal-border']).toBe(
      'var(--line-strong)',
    );
  });

  it('passes dark theme and panel style to Sonner', () => {
    render(<Toaster />);

    const toaster = document.querySelector('[data-sonner-toaster]');
    expect(toaster).toBeTruthy();
    expect(toaster?.getAttribute('data-theme')).toBe('dark');
    expect((toaster as HTMLElement).style.getPropertyValue('--normal-bg')).toBe(
      'var(--surface-2)',
    );
  });
});
