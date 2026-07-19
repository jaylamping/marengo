// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';

import { sheetContentVariants } from '@/components/ui/sheet';

describe('Sheet primitive (panel chrome)', () => {
  it('exports a panel variant on sheet content', () => {
    const classes = sheetContentVariants({ variant: 'panel' });
    expect(classes).toContain('border-line');
    expect(classes).toContain('bg-surface-1');
    expect(classes).not.toContain('backdrop-blur');
  });

  it('keeps default variant opaque for logs/memory compatibility', () => {
    const defaultClasses = sheetContentVariants({ variant: 'default' });
    const panelClasses = sheetContentVariants({ variant: 'panel' });
    expect(defaultClasses).toContain('bg-popover');
    expect(panelClasses).not.toEqual(defaultClasses);
  });
});
