import { useEffect, useRef, useState } from "react";
import { arrayMove } from "@dnd-kit/sortable";
import type { DragEndEvent } from "@dnd-kit/core";
import { toast } from "sonner";
import { isTauriRuntime } from "@/lib/runtime";
import {
  useProviderDisplayPreferencesQuery,
  useProviderOrderQuery,
  useReorderProvidersMutation,
  useSetProviderDisplayPreferencesMutation,
} from "@/lib/query/providers";
import type { Provider, TrayMetric } from "@/types";
import { getErrorMessage } from "./getErrorMessage";

function mergeProviderOrder(
  baseOrder: readonly string[],
  providerIds: readonly string[],
): string[] {
  const knownIds = new Set(providerIds);
  const seen = new Set<string>();
  const merged: string[] = [];

  for (const id of baseOrder) {
    if (!knownIds.has(id) || seen.has(id)) continue;
    seen.add(id);
    merged.push(id);
  }

  for (const id of providerIds) {
    if (seen.has(id)) continue;
    seen.add(id);
    merged.push(id);
  }

  return merged;
}

/**
 * Collapses the `Provider Display Preferences` cluster — card order, card
 * visibility, Windows-taskbar visibility, tray metric and the responsive
 * column count — plus their persistence into one hook returning a small
 * interface. Keeps ProviderPage from re-spreading multiple preference record
 * states at the top level. Mirrors the back-end intent of returning these to
 * the Provider domain (see issue 260624-1509).
 */
export function useProviderDisplayPrefs(providerCatalog: Provider[]) {
  const providerOrderQuery = useProviderOrderQuery();
  const displayPreferencesQuery = useProviderDisplayPreferencesQuery();
  const reorderProviders = useReorderProvidersMutation();
  const setDisplayPreferences = useSetProviderDisplayPreferencesMutation();

  const [order, setOrder] = useState<string[]>(() => providerCatalog.map((p) => p.id));
  const [cardVisible, setCardVisible] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(providerCatalog.map((p) => [p.id, !p.hiddenCard])),
  );
  // Tray visibility is a persisted, opt-in Surface Preference (empty by default),
  // not derived from live status.
  const [trayVisible, setTrayVisible] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(providerCatalog.map((p) => [p.id, false])),
  );
  const [trayMetric, setTrayMetric] = useState<TrayMetric>("Remaining");
  const [colCount, setColCount] = useState(1);
  const gridRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = gridRef.current;
    if (!el) return;
    const update = () =>
      setColCount(Math.max(1, Math.floor((el.clientWidth + 16) / (300 + 16))));
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const providerIds = new Set(providerCatalog.map((provider) => provider.id));
    setOrder((current) =>
      mergeProviderOrder(
        providerOrderQuery.data && providerOrderQuery.data.length > 0
          ? providerOrderQuery.data
          : current,
        providerCatalog.map((provider) => provider.id),
      ).filter((id) => providerIds.has(id)),
    );

    const savedVisible = new Set(displayPreferencesQuery.data?.cardVisibility ?? []);
    const hasSavedVisible = savedVisible.size > 0;
    setCardVisible(
      Object.fromEntries(
        providerCatalog.map((provider) => [
          provider.id,
          hasSavedVisible ? savedVisible.has(provider.id) : !provider.hiddenCard,
        ]),
      ),
    );
    setTrayMetric(displayPreferencesQuery.data?.trayMetric ?? "Remaining");
    const savedTray = new Set(displayPreferencesQuery.data?.trayVisibility ?? []);
    setTrayVisible(
      Object.fromEntries(
        providerCatalog.map((provider) => [provider.id, savedTray.has(provider.id)]),
      ),
    );
  }, [providerCatalog, providerOrderQuery.data, displayPreferencesQuery.data]);

  const nameById = Object.fromEntries(providerCatalog.map((p) => [p.id, p.name]));

  async function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const previousOrder = order;
    const fromIndex = previousOrder.indexOf(String(active.id));
    const toIndex = previousOrder.indexOf(String(over.id));
    if (fromIndex < 0 || toIndex < 0) return;
    const nextOrder = arrayMove(previousOrder, fromIndex, toIndex);

    setOrder(nextOrder);
    if (!isTauriRuntime()) return;

    try {
      await reorderProviders.mutateAsync(nextOrder);
      toast("Provider order saved");
    } catch (error) {
      setOrder(previousOrder);
      toast(getErrorMessage(error));
    }
  }

  async function setCardVisibility(providerId: string, visible: boolean) {
    const next = { ...cardVisible, [providerId]: visible };
    const cardVisibility = order.filter((id) => next[id] !== false);
    const previous = cardVisible;
    const providerName = nameById[providerId] ?? providerId;
    setCardVisible(next);

    const successToast = () =>
      toast(
        visible
          ? `${providerName} shown on Provider page`
          : `${providerName} hidden from Provider page`,
      );

    if (!isTauriRuntime()) {
      successToast();
      return;
    }

    try {
      const saved = await setDisplayPreferences.mutateAsync({
        cardVisibility,
        trayMetric,
        trayVisibility: order.filter((id) => trayVisible[id]),
      });
      const savedVisible = new Set(saved.cardVisibility);
      setTrayMetric(saved.trayMetric);
      setCardVisible((current) => ({
        ...current,
        ...Object.fromEntries(order.map((id) => [id, savedVisible.has(id)])),
      }));
      successToast();
    } catch (error) {
      setCardVisible(previous);
      toast(getErrorMessage(error));
    }
  }

  async function updateTrayMetric(metric: TrayMetric) {
    const previous = trayMetric;
    setTrayMetric(metric);

    if (!isTauriRuntime()) {
      return;
    }

    try {
      const saved = await setDisplayPreferences.mutateAsync({
        cardVisibility: order.filter((id) => cardVisible[id] !== false),
        trayMetric: metric,
        trayVisibility: order.filter((id) => trayVisible[id]),
      });
      setTrayMetric(saved.trayMetric);
    } catch (error) {
      setTrayMetric(previous);
      toast(getErrorMessage(error));
      throw error;
    }
  }

  async function setTrayVisibility(providerId: string, visible: boolean) {
    const next = { ...trayVisible, [providerId]: visible };
    const previous = trayVisible;
    setTrayVisible(next);

    if (!isTauriRuntime()) return;

    try {
      const saved = await setDisplayPreferences.mutateAsync({
        cardVisibility: order.filter((id) => cardVisible[id] !== false),
        trayMetric,
        trayVisibility: order.filter((id) => next[id]),
      });
      const savedTray = new Set(saved.trayVisibility);
      setTrayVisible((current) => ({
        ...current,
        ...Object.fromEntries(order.map((id) => [id, savedTray.has(id)])),
      }));
    } catch (error) {
      setTrayVisible(previous);
      toast(getErrorMessage(error));
      throw error;
    }
  }

  return {
    order,
    cardVisible,
    trayVisible,
    trayMetric,
    colCount,
    gridRef,
    handleDragEnd,
    setCardVisibility,
    setTrayVisibility,
    setTrayMetric: updateTrayMetric,
  };
}
