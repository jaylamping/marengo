// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { SiteHeader } from '@/components/dashboard/site-header/site-header';
import {
  sidebarChromeClassName,
  sidebarGlassSkinClassName,
  sidebarGlassSkinStyle,
  siteHeaderGlassClassName,
} from '@/components/dashboard/sidebar/constants';
import { Sidebar, SidebarProvider } from '@/components/ui/sidebar';

vi.mock('@/hooks/use-mobile', () => ({
  useIsMobile: () => false,
}));

vi.mock('react-router-dom', () => ({
  useMatches: () => [],
}));

afterEach(() => {
  cleanup();
});

describe('sidebar glass skin constants (chrome tier)', () => {
  it('maps sidebar tokens to glass CSS variables', () => {
    expect(sidebarGlassSkinStyle['--sidebar']).toBe('var(--glass-2-surface)');
    expect(sidebarGlassSkinStyle['--sidebar-border']).toBe('var(--glass-border)');
  });

  it('defines a single blur layer with refraction highlight', () => {
    expect(sidebarGlassSkinClassName).toContain('backdrop-blur-xl');
    expect(sidebarGlassSkinClassName).toContain(
      '[border-top-color:var(--glass-refraction-top)]',
    );
  });

  it('raises sidebar chrome above the canvas with pointer events', () => {
    expect(sidebarChromeClassName).toContain('z-30');
    expect(sidebarChromeClassName).toContain('pointer-events-auto');
  });
});

describe('site header glass shell constants', () => {
  it('defines glass chrome with backdrop blur at z-30', () => {
    expect(siteHeaderGlassClassName).toContain('backdrop-blur-xl');
    expect(siteHeaderGlassClassName).toContain('z-30');
    expect(siteHeaderGlassClassName).toContain('pointer-events-auto');
  });

  it('uses glass refraction border distinct from default chrome', () => {
    expect(siteHeaderGlassClassName).toContain(
      '[border-top-color:var(--glass-refraction-top)]',
    );
    expect(siteHeaderGlassClassName).not.toContain('bg-background');
  });
});

describe('Sidebar glass skin (CSS-only, no rewrite)', () => {
  it('applies glass skin to the collapsible-none sidebar shell', () => {
    render(
      <SidebarProvider>
        <Sidebar collapsible="none">
          <span>Nav</span>
        </Sidebar>
      </SidebarProvider>,
    );

    const sidebar = document.querySelector('[data-slot="sidebar"]');
    expect(sidebar).not.toBeNull();
    expect(sidebar?.className).toContain('backdrop-blur-xl');
    expect(sidebar?.className).toContain(
      '[border-top-color:var(--glass-refraction-top)]',
    );
    expect((sidebar as HTMLElement).style.getPropertyValue('--sidebar')).toBe(
      'var(--glass-2-surface)',
    );
  });
});

describe('SiteHeader glass shell', () => {
  it('renders the route title inside a glass header shell', () => {
    render(
      <SidebarProvider>
        <SiteHeader />
      </SidebarProvider>,
    );

    const header = screen.getByRole('banner');
    expect(header.className).toContain('backdrop-blur-xl');
    expect(header.className).toContain('z-30');
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe(
      'Overview',
    );
  });
});
