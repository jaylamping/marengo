import {
  createContext,
  useContext,
  useDeferredValue,
  useMemo,
  useState,
  type ReactNode,
} from 'react';

import type { LogLevelFilter } from '@/data/logs';
import { isChappeLive } from '@/lib/chappe-config';
import {
  DEFAULT_LOG_SORT,
  toggleLogSort,
  type LogSortField,
  type LogSortState,
} from '@/components/dashboard/logs/constants';

type LogsFilterContextValue = {
  levelFilter: LogLevelFilter;
  setLevelFilter: (value: LogLevelFilter) => void;
  searchQuery: string;
  setSearchQuery: (value: string) => void;
  deferredSearchQuery: string;
  sort: LogSortState;
  setSortField: (field: LogSortField) => void;
};

function defaultLevelFilter(): LogLevelFilter {
  return isChappeLive() ? 'INFO' : 'all';
}

const LogsFilterContext = createContext<LogsFilterContextValue | null>(null);

export function LogsFilterProvider({ children }: { children: ReactNode }) {
  const [levelFilter, setLevelFilter] = useState<LogLevelFilter>(defaultLevelFilter);
  const [searchQuery, setSearchQuery] = useState('');
  const [sort, setSort] = useState<LogSortState>(DEFAULT_LOG_SORT);
  const deferredSearchQuery = useDeferredValue(searchQuery);

  const value = useMemo<LogsFilterContextValue>(
    () => ({
      levelFilter,
      setLevelFilter,
      searchQuery,
      setSearchQuery,
      deferredSearchQuery,
      sort,
      setSortField: (field: LogSortField) => {
        setSort((current) => toggleLogSort(current, field));
      },
    }),
    [deferredSearchQuery, levelFilter, searchQuery, sort],
  );

  return (
    <LogsFilterContext.Provider value={value}>{children}</LogsFilterContext.Provider>
  );
}

export function useLogsFilter() {
  const context = useContext(LogsFilterContext);
  if (!context) {
    throw new Error('useLogsFilter must be used within LogsFilterProvider');
  }

  return context;
}
