import type { ReactNode } from 'react';

type MetricGridProps = {
  children: ReactNode;
};

export function MetricGrid({ children }: MetricGridProps) {
  return (
    <dl className="grid grid-cols-2 gap-x-4 gap-y-3">{children}</dl>
  );
}
