import { useMutation, useQueries, useQuery, useQueryClient } from "@tanstack/react-query";

import { providersApi, type ProviderScheduleSettings } from "@/lib/api/providers";
import { isTauriRuntime } from "@/lib/runtime";

export const providerKeys = {
  customCatalog: ["providers", "opencode-custom"] as const,
  displayPreferences: ["providers", "display-preferences"] as const,
  order: ["providers", "order"] as const,
  quota: (providerId: string) => ["providers", providerId, "quota"] as const,
  scheduleSettings: (providerId: string) =>
    ["providers", providerId, "schedule-settings"] as const,
  triggerModels: (providerId: string) =>
    ["providers", providerId, "trigger-models"] as const,
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

export function useProviderDisplayPreferencesQuery() {
  return useQuery({
    queryKey: providerKeys.displayPreferences,
    queryFn: providersApi.getDisplayPreferences,
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

export function useSetProviderDisplayPreferencesMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: providersApi.setDisplayPreferences,
    onSuccess: (preferences) => {
      queryClient.setQueryData(providerKeys.displayPreferences, preferences);
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

export function useProviderQuotaQueries(
  providerIds: readonly string[],
  refreshMsByProvider?: Record<string, number>,
) {
  return useQueries({
    queries: providerIds.map((providerId) => ({
      queryKey: providerKeys.quota(providerId),
      queryFn: () => providersApi.getQuota(providerId),
      enabled: isTauriRuntime(),
      refetchInterval: refreshMsByProvider?.[providerId] ?? PROVIDER_QUOTA_REFETCH_INTERVAL_MS,
      refetchIntervalInBackground: true,
    })),
  });
}

export function useProviderScheduleSettingsQueries(providerIds: readonly string[]) {
  return useQueries({
    queries: providerIds.map((providerId) => ({
      queryKey: providerKeys.scheduleSettings(providerId),
      queryFn: () => providersApi.getProviderScheduleSettings(providerId),
      enabled: isTauriRuntime(),
    })),
  });
}

export function useProviderTriggerModelsQuery(providerId: string | null, open: boolean) {
  return useQuery({
    queryKey: providerKeys.triggerModels(providerId ?? ""),
    queryFn: () => providersApi.listProviderTriggerModels(providerId as string),
    enabled: isTauriRuntime() && open && !!providerId,
    staleTime: 5 * 60 * 1000,
  });
}

export function useSetProviderScheduleSettingsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      providerId,
      settings,
    }: {
      providerId: string;
      settings: ProviderScheduleSettings;
    }) => providersApi.setProviderScheduleSettings(providerId, settings),
    onSuccess: (settings, { providerId }) => {
      queryClient.setQueryData(providerKeys.scheduleSettings(providerId), settings);
    },
  });
}

export function useRunProviderWindowAlignmentMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ providerId, modelId }: { providerId: string; modelId: string }) =>
      providersApi.runProviderWindowAlignment(providerId, modelId),
    onSuccess: (settings, { providerId }) => {
      queryClient.setQueryData(providerKeys.scheduleSettings(providerId), settings);
    },
  });
}
