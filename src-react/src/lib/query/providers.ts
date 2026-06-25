import { useMutation, useQueries, useQuery, useQueryClient } from "@tanstack/react-query";

import { providersApi } from "@/lib/api/providers";
import { isTauriRuntime } from "@/lib/runtime";

export const providerKeys = {
  customCatalog: ["providers", "opencode-custom"] as const,
  order: ["providers", "order"] as const,
  quota: (providerId: string) => ["providers", providerId, "quota"] as const,
};

export const PROVIDER_QUOTA_REFETCH_INTERVAL_MS = 5 * 60 * 1000;

const providerQuotaRefreshOptions = {
  refetchInterval: PROVIDER_QUOTA_REFETCH_INTERVAL_MS,
  refetchIntervalInBackground: true,
} as const;

export function useProviderOrderQuery() {
  return useQuery({
    queryKey: providerKeys.order,
    queryFn: providersApi.getOrder,
    enabled: isTauriRuntime(),
  });
}

export function useReorderProvidersMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (providerIds: string[]) => providersApi.setOrder(providerIds),
    onSuccess: (providerIds: string[]) => {
      queryClient.setQueryData<string[]>(providerKeys.order, providerIds);
    },
  });
}

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
    ...providerQuotaRefreshOptions,
  });
}

export function useProviderQuotaQueries(providerIds: readonly string[]) {
  return useQueries({
    queries: providerIds.map((providerId) => ({
      queryKey: providerKeys.quota(providerId),
      queryFn: () => providersApi.getQuota(providerId),
      enabled: isTauriRuntime(),
      ...providerQuotaRefreshOptions,
    })),
  });
}
