// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';

import { sheetContentVariants } from '@/components/ui/sheet';

describe('Sheet primitive (GLINUI PR4)', () => {
  it('exports a glass variant on sheet content', () => {
    const classes = sheetContentVariants({ variant: 'glass' });
    expect(classes).toContain('backdrop-blur-xl');
    expect(classes).toContain('[border-top-color:var(--glass-refraction-top)]');
  });

  it('keeps default variant opaque for logs sheet compatibility', () => {
    const defaultClasses = sheetContentVariants({ variant: 'default' });
    const glassClasses = sheetContentVariants({ variant: 'glass' });
    expect(defaultClasses).toContain('bg-popover');
    expect(glassClasses).not.toEqual(defaultClasses);
  });
});
