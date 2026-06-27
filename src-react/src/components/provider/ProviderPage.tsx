import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  DndContext,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  SortableContext,
  rectSortingStrategy,
  useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { RefreshCw, Settings } from "lucide-react";
import { toast } from "sonner";
import { Button, IconButton } from "@/components/ui/button";
import { Dot, Input } from "@/components/ui/primitives";
import { Modal, ModalFooter, ModalHeader } from "@/components/ui/modal";
import { Chip, Segmented } from "@/components/ui/segmented";
import { Select } from "@/components/ui/select";
import { Toggle } from "@/components/ui/toggle";
import { ScreenScroll } from "@/components/shell/screen";
import {
  formatProviderQuotaDisplay,
  isQuotaPaceAlert,
} from "@/components/provider/quotaDisplay";
import {
  DEFAULT_QUOTA_REFRESH_MINUTES,
  QUOTA_REFRESH_PRESETS,
  WINDOW_ALIGN_CRON_PRESETS,
  isWindowAlignActive,
  quotaRefreshIntervalMs,
  windowAlignCronHuman,
  windowAlignLastAttemptLabel,
  windowAlignStatusLabel,
} from "@/components/provider/providerSchedule";
import {
  fallbackAgentCapabilities,
  providerRowsFromAgentCapabilities,
} from "@/lib/agentCapabilities";
import { isTauriRuntime } from "@/lib/runtime";
import { builtInProviderRows, customProviderRows } from "@/lib/providerCatalog";
import { useAgentCapabilitiesQuery } from "@/lib/query/agentCapabilities";
import {
  useOpenCodeCustomProvidersQuery,
  useProviderQuotaQueries,
  useProviderScheduleSettingsQueries,
  useProviderTriggerModelsQuery,
  useRunProviderWindowAlignmentMutation,
  useSetProviderScheduleSettingsMutation,
} from "@/lib/query/providers";
import { palette, quotaColor, statusInfo, type ProviderUiStatus } from "@/lib/tokens";
import { cn } from "@/lib/utils";
import type { Provider, TrayMetric } from "@/types";
import type { ProviderQuotaSnapshot } from "@/lib/api/providers";
import { connectionFormFor } from "./connection/registry";
import { getErrorMessage } from "./getErrorMessage";
import { useProviderDisplayPrefs } from "./useProviderDisplayPrefs";

const MSG: Record<string, { title: string; body: string }> = {
  expired: {
    title: "Token expired",
    body: "The saved credential is no longer valid. Re-check to refresh the token.",
  },
  nocreds: {
    title: "No credentials found",
    body: "Add a credential source for this provider to read quota.",
  },
  failed: { title: "Request failed", body: "Quota request failed." },
};

function actionLabel(st: ProviderUiStatus, loading: boolean): string {
  if (st === "expired") return "Re-check";
  if (st === "failed") return "Retry";
  if (st === "nocreds") return "Add cred";
  return loading ? "Checking…" : "Refresh";
}

function mergeProviderQuota(provider: Provider, quota: ProviderQuotaSnapshot): Provider {
  if (provider.id !== quota.providerId) return provider;

  return {
    ...provider,
    status: quota.status,
    plan: quota.plan ?? provider.plan,
    credential: quota.credential ?? provider.credential,
    primary: quota.primary ?? undefined,
    windows:
      quota.windows.length > 0
        ? quota.windows.map((window) => ({
            label: window.label,
            used: window.used,
            valueLabel: window.valueLabel ?? undefined,
            valueOnly: window.valueOnly,
            reset: "",
            kind: window.kind,
            resetAt: window.resetAt ?? undefined,
            unlimited: window.unlimited,
          }))
        : undefined,
    error: quota.error ?? undefined,
  };
}

interface SortableProviderCardProps {
  id: string;
  children: (activator: {
    setActivatorNodeRef: (node: HTMLElement | null) => void;
    listeners: ReturnType<typeof useSortable>["listeners"];
  }) => ReactNode;
}

function SortableProviderCard({
  id,
  children,
}: SortableProviderCardProps) {
  const {
    setNodeRef,
    setActivatorNodeRef,
    listeners,
    attributes,
    transform,
    transition,
    isDragging,
  } = useSortable({ id });
  return (
    <div
      ref={setNodeRef}
      className={cn(
        "flex flex-col rounded-[18px] border bg-nexus-card p-[18px] transition-[box-shadow,opacity]",
        isDragging
          ? "border-nexus-accent opacity-60 shadow-[0_8px_28px_rgba(50,40,25,.16)] z-10"
          : "border-nexus-border shadow-[0_1px_14px_rgba(50,40,25,.05)]",
      )}
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
      }}
      {...attributes}
    >
      {children({ setActivatorNodeRef, listeners })}
    </div>
  );
}

export function ProviderPage() {
  const agentCapabilitiesQuery = useAgentCapabilitiesQuery();
  const customProvidersQuery = useOpenCodeCustomProvidersQuery();
  const baseProviderRows = useMemo(() => {
    const builtInProviders = builtInProviderRows();
    return [
      ...builtInProviders,
      ...customProviderRows(customProvidersQuery.data ?? [], builtInProviders),
    ];
  }, [customProvidersQuery.data]);
  const providerCatalog = useMemo(
    () =>
      providerRowsFromAgentCapabilities(
        agentCapabilitiesQuery.data ?? fallbackAgentCapabilities(),
        baseProviderRows,
      ),
    [agentCapabilitiesQuery.data, baseProviderRows],
  );

  const display = useProviderDisplayPrefs(providerCatalog);

  const [configId, setConfigId] = useState<string | null>(null);
  const [quotaRefreshMinutes, setQuotaRefreshMinutes] = useState(DEFAULT_QUOTA_REFRESH_MINUTES);
  const [windowAlignCron, setWindowAlignCron] = useState("");
  const [windowAlignModelId, setWindowAlignModelId] = useState<string | null>(null);
  const [scheduleSaving, setScheduleSaving] = useState(false);
  const [windowAlignTriggering, setWindowAlignTriggering] = useState(false);
  const [refreshing, setRefreshing] = useState<Record<string, boolean>>({});

  const providerIds = useMemo(
    () => providerCatalog.map((provider) => provider.id),
    [providerCatalog],
  );
  const scheduleResults = useProviderScheduleSettingsQueries(providerIds);
  const scheduleByProvider = Object.fromEntries(
    providerIds.map((providerId, index) => [providerId, scheduleResults[index]?.data]),
  );
  const refreshMsByProvider = Object.fromEntries(
    providerIds.map((providerId) => [
      providerId,
      quotaRefreshIntervalMs(
        scheduleByProvider[providerId]?.quotaRefreshMinutes ?? DEFAULT_QUOTA_REFRESH_MINUTES,
      ),
    ]),
  );
  const quotaResults = useProviderQuotaQueries(providerIds, refreshMsByProvider);
  const quotaQueries = Object.fromEntries(
    providerIds.map((providerId, index) => [providerId, quotaResults[index]]),
  );
  const setProviderScheduleSettings = useSetProviderScheduleSettingsMutation();
  const runProviderWindowAlignment = useRunProviderWindowAlignmentMutation();
  const canUseWindowAlignment = configId === "claude";
  const triggerModelsQuery = useProviderTriggerModelsQuery(configId, canUseWindowAlignment);
  const openSchedule = configId ? scheduleByProvider[configId] : undefined;
  const displayProviders = providerCatalog.map((provider) => {
    const quota = quotaQueries[provider.id]?.data;
    return quota ? mergeProviderQuota(provider, quota) : provider;
  });

  const timers = useRef<Record<string, number>>({});
  useEffect(
    () => () => Object.values(timers.current).forEach((t) => window.clearTimeout(t)),
    [],
  );

  useEffect(() => {
    if (!configId) return;
    setQuotaRefreshMinutes(openSchedule?.quotaRefreshMinutes ?? DEFAULT_QUOTA_REFRESH_MINUTES);
    setWindowAlignCron(openSchedule?.windowAlignCron ?? "");
    setWindowAlignModelId(openSchedule?.windowAlignModelId ?? null);
  }, [configId, openSchedule]);

  const isAnyRefreshing =
    Object.values(quotaQueries).some((q) => q.isFetching) ||
    Object.values(refreshing).some(Boolean);

  async function handleRefreshAll() {
    if (!isTauriRuntime()) {
      toast("Desktop runtime required for refreshing");
      return;
    }
    try {
      await Promise.all(
        Object.entries(quotaQueries).map(([id, q]) => {
          setRefreshing((r) => ({ ...r, [id]: true }));
          window.clearTimeout(timers.current[id]);
          timers.current[id] = window.setTimeout(
            () =>
              setRefreshing((r) => {
                const n = { ...r };
                delete n[id];
                return n;
              }),
            1200,
          );
          return q.refetch();
        }),
      );
      const n = Object.keys(quotaQueries).length;
      toast(`Refreshed ${n} ${n === 1 ? "provider" : "providers"}`);
    } catch {
      /* quota errors surface in-card via ProviderUiStatus */
    }
  }

  function refreshProvider(id: string) {
    void quotaQueries[id]?.refetch();

    setRefreshing((r) => ({ ...r, [id]: true }));
    window.clearTimeout(timers.current[id]);
    timers.current[id] = window.setTimeout(
      () =>
        setRefreshing((r) => {
          const n = { ...r };
          delete n[id];
          return n;
        }),
      1200,
    );
  }

  async function triggerWindowAlignmentNow(providerId: string, modelId: string | null) {
    if (!isTauriRuntime()) {
      toast("Desktop runtime required for window alignment");
      return;
    }
    if (!modelId?.trim()) {
      toast.error("Select a trigger model first");
      return;
    }

    setWindowAlignTriggering(true);
    try {
      await runProviderWindowAlignment.mutateAsync({
        providerId,
        modelId: modelId.trim(),
      });
      toast("Window alignment triggered");
      refreshProvider(providerId);
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setWindowAlignTriggering(false);
    }
  }

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
  );

  const byId = Object.fromEntries(displayProviders.map((p) => [p.id, p]));
  const ordered = display.order.map((id) => byId[id]).filter(Boolean) as Provider[];
  const visible = ordered.filter((p) => display.cardVisible[p.id] !== false);
  const hidden = ordered.filter((p) => display.cardVisible[p.id] === false);
  const cfg = configId ? displayProviders.find((p) => p.id === configId) ?? null : null;
  const triggerSupported = cfg?.id === "claude" && triggerModelsQuery.data?.supported !== false;
  const triggerModels = triggerModelsQuery.data?.models ?? [];
  const modelOptions =
    windowAlignModelId && !triggerModels.some((model) => model.id === windowAlignModelId)
      ? [
          { id: windowAlignModelId, displayName: `${windowAlignModelId} (unavailable)` },
          ...triggerModels,
        ]
      : triggerModels;
  const columns: Provider[][] = Array.from({ length: display.colCount }, () => []);
  visible.forEach((p, i) => columns[i % display.colCount].push(p));

  const connectionForm = cfg ? connectionFormFor(cfg.id, { refreshProvider }) : null;

  return (
    <ScreenScroll>
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="m-0 text-[23px] font-extrabold tracking-[-.02em] text-nexus-ink">
            Provider
          </h1>
          <p className="mt-1.5 text-[13px] text-[#9a8f80]">
            Global quota &amp; credential visibility · {visible.length} of{" "}
            {providerCatalog.length} shown · drag cards to reorder
          </p>
        </div>
        <Button
          variant="subtle"
          size="sm"
          onClick={() => void handleRefreshAll()}
          disabled={isAnyRefreshing}
        >
          <RefreshCw size={14} className={cn(isAnyRefreshing && "animate-spin")} />
          {isAnyRefreshing ? "Refreshing..." : "Refresh all"}
        </Button>
      </div>

      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragEnd={display.handleDragEnd}
      >
        <SortableContext items={visible.map((p) => p.id)} strategy={rectSortingStrategy}>
          <div ref={display.gridRef} className="mt-[22px] flex items-start gap-4">
            {columns.map((col, ci) => (
              <div key={ci} className="flex min-w-0 flex-1 flex-col gap-4">
                {col.map((p) => {
              const loading = !!refreshing[p.id] || !!quotaQueries[p.id]?.isFetching;
              const st: ProviderUiStatus = loading ? "loading" : p.status;
              const si = statusInfo(st);
              const showQuota = st === "available" && !!p.windows;
              const quota = formatProviderQuotaDisplay(p);
              const hasMessage =
                !loading &&
                (p.status === "expired" || p.status === "nocreds" || p.status === "failed");
              const mm = MSG[p.status] ?? { title: "", body: "" };
              const body = p.status === "failed" ? p.error ?? mm.body : mm.body;
              return (
                <SortableProviderCard key={p.id} id={p.id}>
                  {({ setActivatorNodeRef, listeners }) => (
                    <>
                      <div className="flex items-start justify-between gap-2.5">
                        <div className="flex min-w-0 items-start gap-[9px]">
                          <span
                            ref={setActivatorNodeRef}
                            {...listeners}
                            title="Drag to reorder"
                            className="mt-[3px] flex-none cursor-grab text-[13px] leading-none tracking-[-1px] text-[#cabfae]"
                          >
                            ⠿
                          </span>
                          <div className="min-w-0">
                            <div className="flex items-center gap-[7px]">
                              <span className="overflow-hidden text-ellipsis whitespace-nowrap text-[15.5px] font-bold tracking-[-.01em] text-nexus-ink">
                                {p.name}
                              </span>
                              {p.isAgent ? (
                                <span
                                  title="Also a full Agent"
                                  className="flex-none rounded-[5px] border border-nexus-border2 bg-nexus-panel px-[5px] py-[1px] text-[9px] font-bold uppercase tracking-[.06em] text-[#7a6f60]"
                                >
                                  Agent
                                </span>
                              ) : null}
                            </div>
                            <div className="mt-[3px] text-[12px] text-[#a99a89]">{p.plan}</div>
                          </div>
                        </div>
                        <div
                          className="inline-flex flex-none items-center gap-1.5 whitespace-nowrap rounded-full px-2.5 py-1 text-[11px] font-bold"
                          style={{ color: si.color, background: si.color + "22" }}
                        >
                          <Dot color={si.color} dim={7} pulse={loading} />
                          {si.label}
                        </div>
                      </div>

                      {showQuota ? (
                        <>
                          {quota.primaryLabel ? (
                            <div className="mt-[18px] flex items-baseline gap-[7px]">
                              <span
                                className="text-[30px] font-extrabold leading-none tracking-[-.03em]"
                                style={{ color: quotaColor(p.primary ?? 0) }}
                              >
                                {quota.primaryLabel}
                              </span>
                              <span className="text-[12px] text-[#b3a999]">
                                {quota.primaryCaption}
                              </span>
                            </div>
                          ) : null}
                          <div className={cn("flex flex-col gap-[13px]", quota.primaryLabel ? "mt-[15px]" : "mt-[18px]")}>
                            {quota.windows.map((w) => {
                              const barColor = w.unlimited ? quotaColor(0) : quotaColor(w.used);
                              const barWidth = w.unlimited ? 100 : w.used;
                              return (
                                <div key={w.label}>
                                  <div className="mb-1.5 flex justify-between text-[12px]">
                                    <span className="text-[#6a6055]">{w.label}</span>
                                    <span className="font-bold" style={{ color: barColor }}>
                                      {w.usedLabel}
                                    </span>
                                  </div>
                                  {w.valueOnly ? null : (
                                    <div className="relative">
                                      <div className="h-2 overflow-hidden rounded-full bg-[#ece2d6]">
                                        <div
                                          className="h-full rounded-full"
                                          style={{ width: `${barWidth}%`, background: barColor }}
                                        />
                                      </div>
                                      {w.pace != null ? (
                                        <div
                                          className="absolute -top-0.5 h-3 w-0.5 rounded-full"
                                          style={{
                                            left: `calc(${w.pace}% - 1px)`,
                                            background: isQuotaPaceAlert(w)
                                              ? palette.crit
                                              : "#6a6055",
                                          }}
                                        />
                                      ) : null}
                                    </div>
                                  )}
                                  {w.reset ? (
                                    <div className="mt-1.5 text-[11px] text-[#b3a999]">
                                      {w.reset}
                                    </div>
                                  ) : null}
                                </div>
                              );
                            })}
                          </div>
                        </>
                      ) : null}

                      {hasMessage ? (
                        <div
                          className="mt-4 rounded-[13px] px-[14px] py-[13px]"
                          style={{ background: si.color + "22", border: `1px solid ${si.color}44` }}
                        >
                          <div className="text-[12.5px] font-bold" style={{ color: si.color }}>
                            {mm.title}
                          </div>
                          <div className="mt-1 text-[12px] leading-[1.5] text-[#7a6f60]">{body}</div>
                        </div>
                      ) : null}

                      <div className="mt-4 flex items-center justify-between gap-2.5 border-t border-nexus-panel pt-[14px]">
                        <div className="min-w-0">
                          <div className="text-[9.5px] font-bold uppercase tracking-[.08em] text-[#c3b9a8]">
                            Credential
                          </div>
                          <div className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11px] text-[#8a8073]">
                            {p.credential}
                          </div>
                        </div>
                        <div className="flex flex-none gap-1.5">
                          <Button
                            variant="subtle"
                            size="sm"
                            className="px-3"
                            title={actionLabel(st, loading)}
                            onClick={() => refreshProvider(p.id)}
                          >
                            {actionLabel(st, loading)}
                          </Button>
                          <IconButton
                            dim={32}
                            variant="subtle"
                            title="Configure"
                            onClick={() => setConfigId(p.id)}
                          >
                            <Settings size={14} />
                          </IconButton>
                        </div>
                      </div>
                    </>
                  )}
                </SortableProviderCard>
              );
                })}
              </div>
            ))}
          </div>
        </SortableContext>
      </DndContext>

      {hidden.length > 0 ? (
        <div className="mt-5 flex flex-wrap items-center gap-3 rounded-[14px] border border-dashed border-[#ddccb6] bg-nexus-panel px-[18px] py-[14px]">
          <span className="text-[12px] font-bold text-[#7a6f60]">Hidden on this page</span>
          {hidden.map((p) => (
            <div
              key={p.id}
              onClick={() => {
                void display.setCardVisibility(p.id, true);
              }}
              className="inline-flex cursor-pointer items-center gap-1.5 rounded-full border border-nexus-border2 bg-nexus-card px-3 py-[5px] text-[12px] text-[#6a6055] hover:bg-nexus-sand"
            >
              {p.name} <span className="text-[#a99a89]">· show</span>
            </div>
          ))}
          <span className="text-[11px] text-[#b3a999]">
            Card visibility is separate from Windows taskbar visibility.
          </span>
        </div>
      ) : null}

      {/* Tray metric */}
      <div className="mt-5 flex flex-wrap items-center justify-between gap-4 rounded-[14px] border border-nexus-border bg-nexus-sand2 px-[18px] py-[14px]">
        <div className="max-w-[520px]">
          <div className="text-[13px] font-bold text-nexus-body">
            Quota metric shown in the tray icon
          </div>
          <div className="mt-[3px] text-[11.5px] leading-[1.5] text-[#a99a89]">
            Applied globally across all providers so side-by-side icons read consistently.
          </div>
        </div>
        <Segmented<TrayMetric>
          options={[
            { value: "Used", label: "Used" },
            { value: "Remaining", label: "Remaining" },
          ]}
          value={display.trayMetric}
          onChange={(m) => {
            display.setTrayMetric(m);
            toast(`Tray metric set to ${m} (global)`);
          }}
        />
      </div>

      <Modal open={!!cfg} onClose={() => setConfigId(null)} className="max-h-[90vh]">
        {cfg ? (
          <>
            <ModalHeader
              title={`Configure ${cfg.name}`}
              subtitle="Refresh cadence, window alignment & display preferences · not a credential manager"
              onClose={() => setConfigId(null)}
            />
            <div className="flex flex-col gap-[22px] px-[22px] py-5">
              <div>
                <div className="mb-3 text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
                  Connection parameters
                </div>
                {connectionForm ?? (
                  <div className="rounded-[12px] border border-nexus-border bg-nexus-bg px-[14px] py-[13px] text-[12px] leading-[1.5] text-[#8a7a68]">
                    No extra connection parameters needed for {cfg.name} — quota is read from the
                    existing credential source.
                  </div>
                )}
              </div>

              <div>
                <div className="mb-3 text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
                  Display preferences
                </div>
                <div className="flex flex-col gap-0.5">
                  <div
                    onClick={() => {
                      void display.setCardVisibility(cfg.id, display.cardVisible[cfg.id] === false);
                    }}
                    className="flex cursor-pointer items-center justify-between gap-3 rounded-[11px] px-[13px] py-[11px] hover:bg-nexus-sand"
                  >
                    <div>
                      <div className="text-[13px] font-semibold text-nexus-body">
                        Show card on Provider page
                      </div>
                      <div className="mt-0.5 text-[11px] text-[#a99a89]">
                        Affects this page only
                      </div>
                    </div>
                    <Toggle checked={display.cardVisible[cfg.id] !== false} onChange={() => {}} />
                  </div>
                  <div
                    onClick={() => display.toggleTrayVisible(cfg.id)}
                    className="flex cursor-pointer items-center justify-between gap-3 rounded-[11px] px-[13px] py-[11px] hover:bg-nexus-sand"
                  >
                    <div>
                      <div className="text-[13px] font-semibold text-nexus-body">
                        Show in Windows taskbar
                      </div>
                      <div className="mt-0.5 text-[11px] text-[#a99a89]">
                        Separate surface — independent of the card above
                      </div>
                    </div>
                    <Toggle checked={!!display.trayVisible[cfg.id]} onChange={() => {}} />
                  </div>
                </div>
                <div className="mt-2.5 rounded-[11px] border border-nexus-border bg-nexus-bg px-[13px] py-[11px] text-[11.5px] leading-[1.5] text-[#8a7a68]">
                    Taskbar metric (<b className="text-[#6a6055]">used / remaining</b>) is a
                    global setting — configured above the cards on this page. Currently <b className="text-[#6a6055]">{display.trayMetric}</b>.
                </div>
              </div>

              <div>
                <div className="mb-3 text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
                  Quota refresh
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {QUOTA_REFRESH_PRESETS.map((preset) => (
                    <Chip
                      key={preset.minutes}
                      active={quotaRefreshMinutes === preset.minutes}
                      onClick={() => setQuotaRefreshMinutes(preset.minutes)}
                    >
                      {preset.label}
                    </Chip>
                  ))}
                </div>
                <div className="mt-2.5 text-[11px] text-[#b3a999]">
                  How often this card polls {cfg.name} for fresh quota numbers.
                </div>
              </div>

              <div>
                <div className="mb-1 text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
                  Window alignment
                </div>
                <div className="mb-3 text-[11px] leading-[1.5] text-[#a99a89]">
                  Fire a minimal request at set times so the rolling quota window resets on
                  your schedule. Set both a time and a model to turn it on — this makes a
                  real, billable call.
                </div>
                {!triggerSupported ? (
                  <div className="rounded-[12px] border border-nexus-border bg-nexus-bg px-[14px] py-[13px] text-[12px] leading-[1.5] text-[#8a7a68]">
                    Coming soon for {cfg.name} — window alignment is not implemented for this
                    provider yet.
                  </div>
                ) : (
                  <div className="flex flex-col gap-[13px]">
                    <div>
                      <Input
                        className="font-mono"
                        placeholder="0 5,10,15,20 * * *"
                        value={windowAlignCron}
                        onChange={(e) => setWindowAlignCron(e.target.value)}
                      />
                      <div className="mt-2 flex flex-wrap gap-1.5">
                        {WINDOW_ALIGN_CRON_PRESETS.map((preset) => (
                          <Chip
                            key={preset.expr}
                            mono
                            active={windowAlignCron === preset.expr}
                            onClick={() => setWindowAlignCron(preset.expr)}
                          >
                            {preset.expr}
                          </Chip>
                        ))}
                      </div>
                      <div className="mt-2.5 text-[11px] text-[#b3a999]">
                        {windowAlignCronHuman(windowAlignCron)}
                      </div>
                    </div>
                    <div className="block">
                      <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">
                        Trigger model
                      </div>
                      <Select
                        value={windowAlignModelId ?? ""}
                        onChange={(value) => setWindowAlignModelId(value || null)}
                        options={modelOptions.map((model) => ({
                          value: model.id,
                          label:
                            model.displayName === model.id
                              ? model.id
                              : `${model.displayName} · ${model.id}`,
                        }))}
                        placeholder={
                          triggerModelsQuery.isFetching ? "Loading models…" : "Select a model"
                        }
                        disabled={triggerModelsQuery.isFetching}
                      />
                      <div className="mt-[5px] text-[11px] text-[#b3a999]">
                        {isWindowAlignActive(windowAlignCron, windowAlignModelId)
                          ? "Active — alignment fires on the schedule above."
                          : "Inactive — set both a time and a model to enable."}
                      </div>
                    </div>
                    <div className="rounded-[12px] border border-nexus-border bg-nexus-bg px-[14px] py-[13px]">
                      <div className="flex items-center justify-between gap-3">
                        <div className="min-w-0">
                          <div className="text-[12px] font-semibold text-[#6a6055]">
                            Manual trigger
                          </div>
                          <div className="mt-[5px] text-[11px] text-[#b3a999]">
                            Last:{" "}
                            <span className="font-medium text-[#7a6f60]">
                              {windowAlignLastAttemptLabel(
                                openSchedule?.windowAlignLastAttemptAt,
                              )}
                            </span>{" "}
                            ·{" "}
                            <span className="font-medium text-[#7a6f60]">
                              {windowAlignStatusLabel(openSchedule?.windowAlignLastStatus)}
                            </span>
                          </div>
                          {openSchedule?.windowAlignLastError ? (
                            <div className="mt-[5px] overflow-hidden text-ellipsis whitespace-nowrap text-[11px] text-[#b75548]">
                              {openSchedule.windowAlignLastError}
                            </div>
                          ) : null}
                        </div>
                        <Button
                          variant="subtle"
                          size="sm"
                          className="flex-none px-3"
                          disabled={
                            windowAlignTriggering ||
                            !!quotaQueries[cfg.id]?.isFetching ||
                            triggerModelsQuery.isFetching ||
                            !windowAlignModelId?.trim()
                          }
                          onClick={() => void triggerWindowAlignmentNow(cfg.id, windowAlignModelId)}
                        >
                          {windowAlignTriggering ? "Triggering..." : "Trigger now"}
                        </Button>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            </div>
            <ModalFooter>
              <Button variant="subtle" onClick={() => setConfigId(null)}>
                Cancel
              </Button>
              <Button
                variant="primary"
                disabled={scheduleSaving}
                onClick={async () => {
                  const n = cfg.name;
                  if (isTauriRuntime()) {
                    setScheduleSaving(true);
                    try {
                      await setProviderScheduleSettings.mutateAsync({
                        providerId: cfg.id,
                        settings: {
                          quotaRefreshMinutes,
                          windowAlignCron: cfg.id === "claude" ? windowAlignCron.trim() : "",
                          windowAlignModelId: cfg.id === "claude" ? windowAlignModelId : null,
                        },
                      });
                    } catch {
                      toast.error("Failed to save schedule settings");
                      setScheduleSaving(false);
                      return;
                    }
                    setScheduleSaving(false);
                  }
                  setConfigId(null);
                  toast(`Saved preferences for ${n}`);
                }}
              >
                {scheduleSaving ? "Saving..." : "Save"}
              </Button>
            </ModalFooter>
          </>
        ) : null}
      </Modal>
    </ScreenScroll>
  );
}
