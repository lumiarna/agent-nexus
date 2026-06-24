import { useQueries, useQuery } from "@tanstack/react-query";

import { providersApi } from "@/lib/api/providers";
import { isTauriRuntime } from "@/lib/runtime";

export const providerKeys = {
  customCatalog: ["providers", "opencode-custom"] as const,
  quota: (providerId: string) => ["providers", providerId, "quota"] as const,
};

export function useOpenCodeCustomProvidersQuery() {
  return useQuery({
    queryKey: providerKeys.customCatalog,
    queryFn: providersApi.listOpenCodeCustomProviders,
    enabled: isTauriRuntime(),
  });
}

export function useProviderQuotaQuery(providerId: string) {
  return useQuery({
    queryKey: providerKeys.quota(providerId),
    queryFn: () => providersApi.getQuota(providerId),
    enabled: isTauriRuntime(),
  });
}

export function useProviderQuotaQueries(providerIds: readonly string[]) {
  return useQueries({
    queries: providerIds.map((providerId) => ({
      queryKey: providerKeys.quota(providerId),
      queryFn: () => providersApi.getQuota(providerId),
      enabled: isTauriRuntime(),
    })),
  });
}
