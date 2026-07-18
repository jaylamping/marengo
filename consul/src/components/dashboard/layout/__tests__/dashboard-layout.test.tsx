// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import {
  dashboardChromeClassName,
  dashboardLayoutRootClassName,
  dashboardMainPointerClassName,
  sceneBackgroundClassName,
} from '@/components/dashboard/layout/constants';
import { DashboardLayout } from '@/components/dashboard/layout/dashboard-layout';

vi.mock('@/components/dashboard/layout/scene-background', () => ({
  SceneBackground: () => <div data-testid="scene-background" />,
}));

vi.mock('@/components/dashboard/sidebar/app-sidebar', () => ({
  AppSidebar: () => <aside data-testid="app-sidebar" />,
}));

vi.mock('@/components/dashboard/site-header/site-header', () => ({
  SiteHeader: () => <header data-testid="site-header" />,
}));

afterEach(() => {
  cleanup();
});

vi.mock('@/components/ui/sidebar', () => ({
  SidebarProvider: ({
    children,
    className,
    style,
  }: {
    children: React.ReactNode;
    className?: string;
    style?: React.CSSProperties;
  }) => (
    <div data-testid="sidebar-provider" className={className} style={style}>
      {children}
    </div>
  ),
  SidebarInset: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="sidebar-inset">{children}</div>
  ),
}));

describe('Strategy C z-model constants', () => {
  it('defines fullscreen canvas layer at z-0 with pointer-events disabled', () => {
    expect(sceneBackgroundClassName).toContain('fixed');
    expect(sceneBackgroundClassName).toContain('inset-0');
    expect(sceneBackgroundClassName).toContain('z-0');
    expect(sceneBackgroundClassName).toContain('pointer-events-none');
    expect(sceneBackgroundClassName).toContain('will-change-transform');
  });

  it('defines chrome above canvas and main pointer threading', () => {
    expect(dashboardLayoutRootClassName).toContain('relative');
    expect(dashboardLayoutRootClassName).toContain('h-svh');
    expect(dashboardChromeClassName).toContain('z-20');
    expect(dashboardChromeClassName).toContain(
      'has-data-[variant=inset]:bg-transparent',
    );
    expect(dashboardMainPointerClassName).toContain('pointer-events-none');
  });
});

describe('DashboardLayout (Strategy C)', () => {
  it('renders SceneBackground before the chrome layer', () => {
    render(
      <DashboardLayout>
        <p>Panel content</p>
      </DashboardLayout>,
    );

    const background = screen.getByTestId('scene-background');
    const chrome = screen.getByTestId('sidebar-provider');

    expect(background.compareDocumentPosition(chrome)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
  });

  it('applies root and chrome z-model classes', () => {
    render(
      <DashboardLayout>
        <p>Panel content</p>
      </DashboardLayout>,
    );

    const root = screen.getByTestId('dashboard-layout-root');
    const chrome = screen.getByTestId('sidebar-provider');
    const main = screen.getByTestId('dashboard-main');

    expect(root.className).toContain('relative');
    expect(root.className).toContain('h-svh');
    expect(chrome.className).toContain('z-20');
    expect(main.className).toContain('pointer-events-none');
  });

  it('still renders route children inside the main shell', () => {
    render(
      <DashboardLayout>
        <p>Panel content</p>
      </DashboardLayout>,
    );

    expect(screen.getByText('Panel content')).toBeTruthy();
  });
});
