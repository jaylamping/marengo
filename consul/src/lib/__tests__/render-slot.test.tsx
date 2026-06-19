// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { useRender } from '@/lib/render-slot';

function AnchorButton({
  label,
  href,
}: {
  label: string;
  href: string;
}) {
  return useRender({
    defaultTagName: 'button',
    props: {
      children: [<span key="label">{label}</span>],
    },
    render: <a href={href} />,
    state: { slot: 'test-anchor-button' },
  });
}

describe('useRender', () => {
  it('preserves children when merging onto a render element', () => {
    render(<AnchorButton label="Overview" href="/" />);

    const link = screen.getByRole('link', { name: 'Overview' });
    expect(link).toBeTruthy();
    expect(link.getAttribute('href')).toBe('/');
    expect(link.getAttribute('data-slot')).toBe('test-anchor-button');
  });
});
