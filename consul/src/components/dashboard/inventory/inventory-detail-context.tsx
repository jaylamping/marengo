import { createContext, useContext, type ReactNode } from 'react';

import type { InventoryRow } from '@/components/dashboard/inventory/types';

type InventoryDetailContextValue = {
  openItem: (item: InventoryRow) => void;
};

const InventoryDetailContext = createContext<InventoryDetailContextValue | null>(
  null,
);

export function InventoryDetailProvider({
  openItem,
  children,
}: {
  openItem: (item: InventoryRow) => void;
  children: ReactNode;
}) {
  return (
    <InventoryDetailContext.Provider value={{ openItem }}>
      {children}
    </InventoryDetailContext.Provider>
  );
}

export function useInventoryDetail(): InventoryDetailContextValue {
  const value = useContext(InventoryDetailContext);
  if (!value) {
    throw new Error('useInventoryDetail requires InventoryDetailProvider');
  }
  return value;
}
