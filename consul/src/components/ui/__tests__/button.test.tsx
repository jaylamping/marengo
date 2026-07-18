// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';

import { Button, buttonVariants } from '@/components/ui/button';

describe('Button (Launch Day panel chrome)', () => {
  it('applies panel variant classes from buttonVariants', () => {
    const classes = buttonVariants({ variant: 'panel' });
    expect(classes).toContain('border-line');
    expect(classes).toContain('bg-surface-2/50');
    expect(classes).not.toContain('backdrop-blur');
  });

  it('renders panel variant with accessible label', () => {
    render(<Button variant="panel">Enable hold</Button>);
    const control = screen.getByRole('button', { name: 'Enable hold' });
    expect(control.dataset.slot).toBe('button');
    expect(control.className).toContain('border-line');
  });

  it('does not apply panel styling to default variant', () => {
    const defaultClasses = buttonVariants({ variant: 'default' });
    const panelClasses = buttonVariants({ variant: 'panel' });
    expect(defaultClasses).toContain('bg-primary');
    expect(panelClasses).not.toEqual(defaultClasses);
  });
});
