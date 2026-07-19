// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { SiteHeader } from '@/components/dashboard/site-header/site-header';
import {
  sidebarChromeClassName,
  sidebarPanelSkinClassName,
  sidebarPanelSkinStyle,
  siteHeaderPanelClassName,
} from '@/components/dashboard/sidebar/constants';
import { Sidebar, SidebarProvider } from '@/components/ui/sidebar';

vi.mock('@/hooks/use-mobile', () => ({
  useIsMobile: () => false,
}));

vi.mock('react-router-dom', () => ({
  useMatches: () => [],
}));

vi.mock('@/lib/chappe-config', () => ({
  isChappeLive: () => false,
}));

afterEach(() => {
  cleanup();
});

describe('sidebar panel skin constants (chrome tier)', () => {
  it('maps sidebar tokens to opaque surface CSS variables', () => {
    expect(sidebarPanelSkinStyle['--sidebar']).toBe('var(--surface-1)');
    expect(sidebarPanelSkinStyle['--sidebar-border']).toBe('var(--line)');
    expect(sidebarPanelSkinStyle['--sidebar-foreground']).toBe('var(--foreground)');
  });

  it('defines an opaque panel with hairline border and no blur', () => {
    expect(sidebarPanelSkinClassName).toContain('border-line');
    expect(sidebarPanelSkinClassName).toContain('bg-surface-1');
    expect(sidebarPanelSkinClassName).not.toContain('backdrop-blur');
  });

  it('raises sidebar chrome above the canvas with pointer events', () => {
    expect(sidebarChromeClassName).toContain('z-30');
    expect(sidebarChromeClassName).toContain('pointer-events-auto');
  });
});

describe('site header panel shell constants', () => {
  it('defines opaque panel chrome at z-30', () => {
    expect(siteHeaderPanelClassName).toContain('bg-surface-1');
    expect(siteHeaderPanelClassName).toContain('border-b');
    expect(siteHeaderPanelClassName).toContain('z-30');
    expect(siteHeaderPanelClassName).toContain('pointer-events-auto');
    expect(siteHeaderPanelClassName).not.toContain('backdrop-blur');
  });
});

describe('Sidebar panel skin (CSS-only, no rewrite)', () => {
  it('applies panel skin to the collapsible-none sidebar shell', () => {
    render(
      <SidebarProvider>
        <Sidebar collapsible="none">
          <span>Nav</span>
        </Sidebar>
      </SidebarProvider>,
    );

    const sidebar = document.querySelector('[data-slot="sidebar"]');
    expect(sidebar).not.toBeNull();
    expect(sidebar?.className).toContain('bg-surface-1');
    expect(sidebar?.className).not.toContain('backdrop-blur');
    expect((sidebar as HTMLElement).style.getPropertyValue('--sidebar')).toBe(
      'var(--surface-1)',
    );
  });
});

describe('SiteHeader panel shell', () => {
  it('renders the route title as an instrument legend inside the panel header', () => {
    render(
      <SidebarProvider>
        <SiteHeader />
      </SidebarProvider>,
    );

    const header = screen.getByRole('banner');
    expect(header.className).toContain('bg-surface-1');
    expect(header.className).toContain('z-30');
    expect(header.className).not.toContain('backdrop-blur');
    const heading = screen.getByRole('heading', { level: 1 });
    expect(heading.textContent).toBe('Overview');
    expect(heading.className).toContain('micro-label');
  });

  it('renders the dominant machine-state readout', () => {
    render(
      <SidebarProvider>
        <SiteHeader />
      </SidebarProvider>,
    );

    expect(screen.getByTestId('machine-state').textContent).toBe('WIREFRAME');
  });
});
