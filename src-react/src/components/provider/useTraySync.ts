import { useEffect } from "react";

import { providersApi, type TrayEntry } from "@/lib/api/providers";
import {
  useProviderDisplayPreferencesQuery,
  useProviderQuotaQueries,
} from "@/lib/query/providers";
import { isTauriRuntime } from "@/lib/runtime";
import { providerBrand } from "@/lib/tokens";

/**
 * Keeps the Windows-taskbar tray in sync with the tray-visible `Provider`s'
 * quota, app-wide and independent of which page is mounted. Mounted once at the
 * App root so the tray stays live even when the window is hidden to tray.
 *
 * Each entry paints one icon: the provider's brand colour with its
 * "shortest window used" (`primary`) rendered under the global Used/Remaining
 * metric. A provider with no live `primary` contributes no icon (nothing to
 * show); an empty entry set clears the tray.
 */
export function useTraySync() {
  const displayPreferencesQuery = useProviderDisplayPreferencesQuery();
  const trayIds = displayPreferencesQuery.data?.trayVisibility ?? [];
  const trayMetric = displayPreferencesQuery.data?.trayMetric ?? "Remaining";
  const quotaResults = useProviderQuotaQueries(trayIds);

  const entries: TrayEntry[] = trayIds.flatMap((providerId, index) => {
    const primary = quotaResults[index]?.data?.primary;
    if (primary == null) return [];
    const brand = providerBrand(providerId);
    const value = trayMetric === "Remaining" ? 100 - primary : primary;
    return [{ providerId, label: brand.name, colorHex: brand.color, value }];
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
