import { useEffect } from "react";

import { providersApi, type TrayEntry } from "@/lib/api/providers";
import {
  useProviderDisplayPreferencesQuery,
  useProviderQuotaQueries,
  useProviderScheduleSettingsQueries,
} from "@/lib/query/providers";
import { isTauriRuntime } from "@/lib/runtime";
import { providerBrand } from "@/lib/tokens";

import { quotaRefreshIntervalMs } from "./providerSchedule";

/**
 * Keeps the Windows-taskbar tray in sync with the tray-visible `Provider`s'
 * quota, app-wide and independent of which page is mounted. Mounted once at the
 * App root so the tray stays live even when the window is hidden to tray.
 *
 * Each entry paints one icon: the provider's brand colour with its
 * "shortest window used" (`primary`) rendered under the global Used/Remaining
 * metric. A failed quota fetch contributes a failure marker; a successful
 * provider with no live `primary` contributes no icon (nothing to show).
 */
export function useTraySync() {
  const displayPreferencesQuery = useProviderDisplayPreferencesQuery();
  const trayIds = displayPreferencesQuery.data?.trayVisibility ?? [];
  const trayMetric = displayPreferencesQuery.data?.trayMetric ?? "Remaining";
  const scheduleResults = useProviderScheduleSettingsQueries(trayIds);
  const refreshMsByProvider = Object.fromEntries(
    trayIds.map((providerId, index) => [
      providerId,
      quotaRefreshIntervalMs(scheduleResults[index]?.data?.quotaRefreshMinutes),
    ]),
  );
  const quotaResults = useProviderQuotaQueries(trayIds, refreshMsByProvider);

  const entries: TrayEntry[] = trayIds.flatMap((providerId, index) => {
    const result = quotaResults[index];
    const primary = result?.data?.primary;
    if (result?.isError || result?.data?.status === "failed") {
      const brand = providerBrand(providerId);
      const entry: TrayEntry = {
        providerId,
        label: brand.name,
        colorHex: brand.color,
        value: null,
      };
      return [entry];
    }
    if (primary == null) return [];
    const brand = providerBrand(providerId);
    const value = trayMetric === "Remaining" ? 100 - primary : primary;
    const entry: TrayEntry = { providerId, label: brand.name, colorHex: brand.color, value };
    return [entry];
  });

  // Serialised so the effect only fires on a real change (numbers, colours, set).
  const key = JSON.stringify(entries);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    void providersApi.syncTray(entries).catch(() => {});
    // `entries` is derived from `key`; depend on `key` to avoid ref churn.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
}
