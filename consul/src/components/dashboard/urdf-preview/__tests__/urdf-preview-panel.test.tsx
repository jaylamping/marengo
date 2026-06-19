// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { urdfPreviewPanelClassName } from '@/components/dashboard/layout/constants';
import { UrdfPreviewPanel } from '@/components/dashboard/urdf-preview/urdf-preview-panel';

afterEach(() => {
  cleanup();
});

describe('UrdfPreviewPanel (Strategy C overlay)', () => {
  it('does not use the legacy 400px bordered widget shell', () => {
    render(<UrdfPreviewPanel />);

    const panel = screen.getByTestId('urdf-preview-panel');
    expect(panel.className).not.toMatch(/h-\[400px\]/);
    expect(panel.className).not.toMatch(/\bborder\b/);
    expect(panel.className).toBe(urdfPreviewPanelClassName);
  });

  it('acts as a transparent framing overlay for the shared canvas', () => {
    render(<UrdfPreviewPanel />);

    const panel = screen.getByTestId('urdf-preview-panel');
    expect(panel.className).toContain('pointer-events-none');
    expect(panel.className).toContain('bg-transparent');
  });
});
