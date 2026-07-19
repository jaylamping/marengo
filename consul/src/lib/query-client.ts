import { QueryClient } from '@tanstack/react-query';
import { createSyncStoragePersister } from '@tanstack/query-sync-storage-persister';
import type {
  PersistQueryClientOptions,
  Persister,
} from '@tanstack/react-query-persist-client';
import { defaultShouldDehydrateQuery } from '@tanstack/react-query';

import { isPersistableQueryKey } from '@/lib/query-keys';

/** Keep in sync with PersistQueryClientProvider maxAge. */
export const QUERY_CACHE_MAX_AGE_MS = 1000 * 60 * 60 * 24;

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Required for persist: must be >= maxAge or restored cache is GC'd early.
      gcTime: QUERY_CACHE_MAX_AGE_MS,
      retry: 1,
    },
  },
});

function createLocalStoragePersister(): Persister | null {
  if (typeof window === 'undefined') {
    return null;
  }
  try {
    return createSyncStoragePersister({
      storage: window.localStorage,
      key: 'marengo-consul-query-cache',
      throttleTime: 1000,
    });
  } catch {
    return null;
  }
}

export const queryPersister = createLocalStoragePersister();

export type ConsulPersistOptions = Omit<PersistQueryClientOptions, 'queryClient'>;

export const persistOptions: ConsulPersistOptions | null = queryPersister
  ? {
      persister: queryPersister,
      maxAge: QUERY_CACHE_MAX_AGE_MS,
      buster: 'consul-query-v2',
      dehydrateOptions: {
        shouldDehydrateQuery: (query) =>
          defaultShouldDehydrateQuery(query) && isPersistableQueryKey(query.queryKey),
      },
    }
  : null;
