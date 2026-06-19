// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';

import { Button, buttonVariants } from '@/components/ui/button';

describe('Button (GLINUI foundation)', () => {
  it('applies glass variant classes from buttonVariants', () => {
    const classes = buttonVariants({ variant: 'glass' });
    expect(classes).toContain('backdrop-blur-xl');
    expect(classes).toContain('[border-top-color:var(--glass-refraction-top)]');
  });

  it('renders glass variant with accessible label', () => {
    render(<Button variant="glass">Enable hold</Button>);
    const control = screen.getByRole('button', { name: 'Enable hold' });
    expect(control.dataset.slot).toBe('button');
    expect(control.className).toMatch(/backdrop-blur-xl/);
  });

  it('does not apply glass styling to default variant', () => {
    const defaultClasses = buttonVariants({ variant: 'default' });
    const glassClasses = buttonVariants({ variant: 'glass' });
    expect(defaultClasses).not.toContain('backdrop-blur-xl');
    expect(glassClasses).not.toEqual(defaultClasses);
  });
});
