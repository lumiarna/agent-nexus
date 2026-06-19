import { useQuery } from "@tanstack/react-query";

import { providersApi } from "@/lib/api/providers";
import { isTauriRuntime } from "@/lib/runtime";

export const providerKeys = {
  quota: (providerId: string) => ["providers", providerId, "quota"] as const,
};

export function useProviderQuotaQuery(providerId: string) {
  return useQuery({
    queryKey: providerKeys.quota(providerId),
    queryFn: () => providersApi.getQuota(providerId),
    enabled: isTauriRuntime(),
  });
}
