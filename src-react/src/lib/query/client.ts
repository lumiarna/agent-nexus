import { QueryClient } from "@tanstack/react-query";

export const APP_QUERY_STALE_TIME = Infinity;

export function createAppQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        refetchOnWindowFocus: false,
        retry: false,
        staleTime: APP_QUERY_STALE_TIME,
      },
    },
  });
}
