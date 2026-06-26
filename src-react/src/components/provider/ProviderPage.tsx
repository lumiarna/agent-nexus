import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  DndContext,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  rectSortingStrategy,
  useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { RefreshCw, Settings } from "lucide-react";
import { toast } from "sonner";
import { Button, IconButton } from "@/components/ui/button";
import { Dot, Input } from "@/components/ui/primitives";
import { Modal, ModalFooter, ModalHeader } from "@/components/ui/modal";
import { Segmented } from "@/components/ui/segmented";
import { Toggle } from "@/components/ui/toggle";
import { ScreenScroll } from "@/components/shell/screen";
import {
  formatProviderQuotaDisplay,
  isQuotaPaceAlert,
} from "@/components/provider/quotaDisplay";
import { nexus } from "@/lib/mock";
import { providersApi } from "@/lib/api/providers";
import {
  fallbackAgentCapabilities,
  providerRowsFromAgentCapabilities,
} from "@/lib/agentCapabilities";
import { isTauriRuntime } from "@/lib/runtime";
import { customProviderRows } from "@/lib/providerCatalog";
import { useAgentCapabilitiesQuery } from "@/lib/query/agentCapabilities";
import {
  useOpenCodeCustomProvidersQuery,
  useProviderDisplayPreferencesQuery,
  useProviderOrderQuery,
  useProviderQuotaQueries,
  useReorderProvidersMutation,
  useSetProviderDisplayPreferencesMutation,
} from "@/lib/query/providers";
import { palette, quotaColor, statusInfo, type ProviderUiStatus } from "@/lib/tokens";
import { cn } from "@/lib/utils";
import type { Provider, TrayMetric } from "@/types";
import type { ProviderQuotaSnapshot } from "@/lib/api/providers";

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

const API_KEY_PROVIDER_HINTS: Record<
  string,
  { placeholder: string; authKey: string; savedLabel: string }
> = {
  "minimax-token": {
    placeholder: "sk-...",
    authKey: "minimax-cn-coding-plan",
    savedLabel: "MiniMax Token Plan CN API key",
  },
  deepseek: {
    placeholder: "sk-...",
    authKey: "deepseek",
    savedLabel: "DeepSeek API key",
  },
  openrouter: {
    placeholder: "sk-or-v1-...",
    authKey: "openrouter",
    savedLabel: "OpenRouter API key",
  },
};

function isApiKeyProvider(providerId: string): boolean {
  return providerId in API_KEY_PROVIDER_HINTS;
}

function actionLabel(st: ProviderUiStatus, loading: boolean): string {
  if (st === "expired") return "Re-check";
  if (st === "failed") return "Retry";
  if (st === "nocreds") return "Add cred";
  return loading ? "Checking…" : "Refresh";
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (
    error &&
    typeof error === "object" &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }
  return "Unexpected error";
}

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
  const providerDisplayPreferencesQuery = useProviderDisplayPreferencesQuery();
  const providerOrderQuery = useProviderOrderQuery();
  const reorderProviders = useReorderProvidersMutation();
  const setProviderDisplayPreferences = useSetProviderDisplayPreferencesMutation();
  const providerSeeds = useMemo(() => {
    const builtInProviders = nexus.providers();
    return [
      ...builtInProviders,
      ...customProviderRows(customProvidersQuery.data ?? [], builtInProviders),
    ];
  }, [customProvidersQuery.data]);
  const providerCatalog = useMemo(
    () =>
      providerRowsFromAgentCapabilities(
        agentCapabilitiesQuery.data ?? fallbackAgentCapabilities(),
        providerSeeds,
      ),
    [agentCapabilitiesQuery.data, providerSeeds],
  );
  const [order, setOrder] = useState<string[]>(() => providerCatalog.map((p) => p.id));
  const [cardVisible, setCardVisible] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(providerCatalog.map((p) => [p.id, !p.hiddenCard])),
  );
  const [trayVisible, setTrayVisible] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(providerCatalog.map((p) => [p.id, p.status === "available"])),
  );
  const [configId, setConfigId] = useState<string | null>(null);
  const [copilotToken, setCopilotToken] = useState("");
  const [copilotTokenSaving, setCopilotTokenSaving] = useState(false);
  const [opencodeGoWorkspaceId, setOpencodeGoWorkspaceId] = useState("");
  const [opencodeGoAuthCookie, setOpencodeGoAuthCookie] = useState("");
  const [opencodeGoSaving, setOpencodeGoSaving] = useState(false);
  const [providerApiKey, setProviderApiKey] = useState("");
  const [providerApiKeySaving, setProviderApiKeySaving] = useState(false);
  const [refreshing, setRefreshing] = useState<Record<string, boolean>>({});
  const [trayMetric, setTrayMetric] = useState<TrayMetric>(() => nexus.settings().trayMetric);
  const [colCount, setColCount] = useState(1);
  const providerIds = useMemo(
    () => providerCatalog.map((provider) => provider.id),
    [providerCatalog],
  );
  const quotaResults = useProviderQuotaQueries(providerIds);
  const quotaQueries = Object.fromEntries(
    providerIds.map((providerId, index) => [providerId, quotaResults[index]]),
  );
  const displayProviders = providerCatalog.map((provider) => {
    const quota = quotaQueries[provider.id]?.data;
    return quota ? mergeProviderQuota(provider, quota) : provider;
  });

  const gridRef = useRef<HTMLDivElement>(null);
  const timers = useRef<Record<string, number>>({});
  useEffect(
    () => () => Object.values(timers.current).forEach((t) => window.clearTimeout(t)),
    [],
  );

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

    const savedVisible = new Set(providerDisplayPreferencesQuery.data?.cardVisibility ?? []);
    const hasSavedVisible = savedVisible.size > 0;
    setCardVisible(
      Object.fromEntries(
        providerCatalog.map((provider) => [
          provider.id,
          hasSavedVisible ? savedVisible.has(provider.id) : !provider.hiddenCard,
        ]),
      ),
    );
    setTrayVisible((current) => ({
      ...Object.fromEntries(
        providerCatalog.map((provider) => [provider.id, provider.status === "available"]),
      ),
      ...current,
    }));
  }, [providerCatalog, providerOrderQuery.data, providerDisplayPreferencesQuery.data]);

  useEffect(() => {
    if (configId !== "copilot" || !isTauriRuntime()) return;
    let active = true;
    providersApi
      .getCopilotGithubToken()
      .then((token) => {
        if (active) setCopilotToken(token ?? "");
      })
      .catch(() => {
        if (active) setCopilotToken("");
      });
    return () => {
      active = false;
    };
  }, [configId]);

  useEffect(() => {
    if (configId !== "opencode-go" || !isTauriRuntime()) return;
    let active = true;
    providersApi
      .getOpenCodeGoConnectionParams()
      .then((params) => {
        if (!active) return;
        setOpencodeGoWorkspaceId(params.workspaceId);
        setOpencodeGoAuthCookie(params.authCookie);
      })
      .catch(() => {
        if (!active) return;
        setOpencodeGoWorkspaceId("");
        setOpencodeGoAuthCookie("");
      });
    return () => {
      active = false;
    };
  }, [configId]);

  useEffect(() => {
    if (!configId || !isApiKeyProvider(configId) || !isTauriRuntime()) return;
    let active = true;
    providersApi
      .getProviderConnectionParams(configId)
      .then((params) => {
        if (active) setProviderApiKey(params.apiKey);
      })
      .catch(() => {
        if (active) setProviderApiKey("");
      });
    return () => {
      active = false;
    };
  }, [configId]);

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

  function reorder(fromId: string, toId: string, currentOrder: string[]) {
    if (!fromId || fromId === toId) return null;
    const fromIndex = currentOrder.indexOf(fromId);
    const toIndex = currentOrder.indexOf(toId);
    if (fromIndex < 0 || toIndex < 0) return null;
    return arrayMove(currentOrder, fromIndex, toIndex);
  }

  function nextCardVisibility(providerId: string, visible: boolean) {
    const next = { ...cardVisible, [providerId]: visible };
    const cardVisibility = order.filter((id) => next[id] !== false);
    return { next, cardVisibility };
  }


  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
  );

  async function handleDragEnd(e: DragEndEvent) {
    const { active, over } = e;
    if (!over || active.id === over.id) return;
    const previousOrder = order;
    const nextOrder = reorder(String(active.id), String(over.id), previousOrder);
    if (!nextOrder) return;

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

  const byId = Object.fromEntries(displayProviders.map((p) => [p.id, p]));
  const ordered = order.map((id) => byId[id]).filter(Boolean) as Provider[];
  const visible = ordered.filter((p) => cardVisible[p.id] !== false);
  const hidden = ordered.filter((p) => cardVisible[p.id] === false);
  const cfg = configId ? displayProviders.find((p) => p.id === configId) ?? null : null;
  const columns: Provider[][] = Array.from({ length: colCount }, () => []);
  visible.forEach((p, i) => columns[i % colCount].push(p));

  async function persistCardVisibility(providerId: string, visible: boolean) {
    const { next, cardVisibility } = nextCardVisibility(providerId, visible);
    const previous = cardVisible;
    const providerName = byId[providerId]?.name ?? providerId;
    setCardVisible(next);

    if (!isTauriRuntime()) {
      toast(
        visible
          ? `${providerName} shown on Provider page`
          : `${providerName} hidden from Provider page`,
      );
      return;
    }

    try {
      const saved = await setProviderDisplayPreferences.mutateAsync({ cardVisibility });
      const savedVisible = new Set(saved.cardVisibility);
      setCardVisible((current) => ({
        ...current,
        ...Object.fromEntries(order.map((id) => [id, savedVisible.has(id)])),
      }));
      toast(
        visible
          ? `${providerName} shown on Provider page`
          : `${providerName} hidden from Provider page`,
      );
    } catch (error) {
      setCardVisible(previous);
      toast(getErrorMessage(error));
    }
  }


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
        onDragEnd={handleDragEnd}
      >
        <SortableContext items={visible.map((p) => p.id)} strategy={rectSortingStrategy}>
          <div ref={gridRef} className="mt-[22px] flex items-start gap-4">
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
                void persistCardVisibility(p.id, true);
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
          value={trayMetric}
          onChange={(m) => {
            setTrayMetric(m);
            toast(`Tray metric set to ${m} (global)`);
          }}
        />
      </div>

      <Modal open={!!cfg} onClose={() => setConfigId(null)}>
        {cfg ? (
          <>
            <ModalHeader
              title={`Configure ${cfg.name}`}
              subtitle="Observation params & display preferences · not a credential manager"
              onClose={() => setConfigId(null)}
            />
            <div className="flex flex-col gap-[22px] px-[22px] py-5">
              <div>
                <div className="mb-3 text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
                  Connection parameters
                </div>
                {cfg.id === "copilot" ? (
                  <label className="block">
                    <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">
                      GitHub token
                    </div>
                    <Input
                      type="password"
                      className="font-mono"
                      placeholder="gho_... or ghp_..."
                      value={copilotToken}
                      onChange={(e) => setCopilotToken(e.target.value)}
                    />
                    <div className="mt-[5px] text-[11px] text-[#b3a999]">
                      A Copilot-scoped GitHub token used only to read quota. Leave empty to
                      fall back to opencode&apos;s <span className="font-mono">auth.json</span>{" "}
                      (<span className="font-mono">github-copilot</span>).
                    </div>
                  </label>
                ) : isApiKeyProvider(cfg.id) ? (
                  <label className="block">
                    <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">
                      API key
                    </div>
                    <Input
                      type="password"
                      className="font-mono"
                      placeholder={API_KEY_PROVIDER_HINTS[cfg.id].placeholder}
                      value={providerApiKey}
                      onChange={(e) => setProviderApiKey(e.target.value)}
                    />
                    <div className="mt-[5px] text-[11px] text-[#b3a999]">
                      Used only to read quota. Leave empty to fall back to opencode&apos;s{" "}
                      <span className="font-mono">auth.json</span> (
                      <span className="font-mono">
                        {API_KEY_PROVIDER_HINTS[cfg.id].authKey}
                      </span>
                      ).
                    </div>
                  </label>
                ) : cfg.id === "opencode-go" ? (
                  <div className="flex flex-col gap-[13px]">
                    <label className="block">
                      <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">
                        Workspace ID
                      </div>
                      <Input
                        className="font-mono"
                        placeholder="wrk_xxxxxxxxxxxx"
                        value={opencodeGoWorkspaceId}
                        onChange={(e) => setOpencodeGoWorkspaceId(e.target.value)}
                      />
                      <div className="mt-[5px] text-[11px] text-[#b3a999]">
                        Required to query the workspace quota endpoint.
                      </div>
                    </label>
                    <label className="block">
                      <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">
                        Auth Cookie
                      </div>
                      <Input
                        type="password"
                        className="font-mono"
                        placeholder="Fe26.2**..."
                        value={opencodeGoAuthCookie}
                        onChange={(e) => setOpencodeGoAuthCookie(e.target.value)}
                      />
                      <div className="mt-[5px] text-[11px] text-[#b3a999]">
                        Paste only the auth cookie value from opencode.ai.
                      </div>
                    </label>
                  </div>
                ) : (
                  <div className="rounded-[12px] border border-nexus-border bg-nexus-bg px-[14px] py-[13px] text-[12px] leading-[1.5] text-[#8a7a68]">
                    No extra connection parameters needed for {cfg.name} — quota is read
                    from the existing credential source.
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
                      void persistCardVisibility(cfg.id, cardVisible[cfg.id] === false);
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
                    <Toggle checked={cardVisible[cfg.id] !== false} onChange={() => {}} />
                  </div>
                  <div
                    onClick={() =>
                      setTrayVisible((tv) => ({ ...tv, [cfg.id]: !tv[cfg.id] }))
                    }
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
                    <Toggle checked={!!trayVisible[cfg.id]} onChange={() => {}} />
                  </div>
                </div>
                <div className="mt-2.5 rounded-[11px] border border-nexus-border bg-nexus-bg px-[13px] py-[11px] text-[11.5px] leading-[1.5] text-[#8a7a68]">
                    Taskbar metric (<b className="text-[#6a6055]">used / remaining</b>) is a
                    global setting — configured above the cards on this page. Currently <b className="text-[#6a6055]">{trayMetric}</b>.
                </div>
              </div>
            </div>
            <ModalFooter>
              <Button variant="subtle" onClick={() => setConfigId(null)}>
                Cancel
              </Button>
              <Button
                variant="primary"
                disabled={copilotTokenSaving || opencodeGoSaving || providerApiKeySaving}
                onClick={async () => {
                  const n = cfg.name;
                  if (cfg.id === "copilot") {
                    setCopilotTokenSaving(true);
                    try {
                      await providersApi.setCopilotGithubToken(copilotToken.trim());
                      setConfigId(null);
                      toast("Saved Copilot GitHub token");
                      refreshProvider("copilot");
                    } catch {
                      toast.error("Failed to save Copilot GitHub token");
                    } finally {
                      setCopilotTokenSaving(false);
                    }
                    return;
                  }
                  if (isApiKeyProvider(cfg.id)) {
                    setProviderApiKeySaving(true);
                    try {
                      await providersApi.setProviderConnectionParams(cfg.id, {
                        apiKey: providerApiKey.trim(),
                      });
                      setConfigId(null);
                      toast(`Saved ${API_KEY_PROVIDER_HINTS[cfg.id].savedLabel}`);
                      refreshProvider(cfg.id);
                    } catch {
                      toast.error(`Failed to save ${API_KEY_PROVIDER_HINTS[cfg.id].savedLabel}`);
                    } finally {
                      setProviderApiKeySaving(false);
                    }
                    return;
                  }
                  if (cfg.id === "opencode-go") {
                    setOpencodeGoSaving(true);
                    try {
                      await providersApi.setOpenCodeGoConnectionParams({
                        workspaceId: opencodeGoWorkspaceId.trim(),
                        authCookie: opencodeGoAuthCookie.trim(),
                      });
                      setConfigId(null);
                      toast("Saved OpenCode Go connection params");
                      refreshProvider("opencode-go");
                    } catch {
                      toast.error("Failed to save OpenCode Go connection params");
                    } finally {
                      setOpencodeGoSaving(false);
                    }
                    return;
                  }
                  setConfigId(null);
                  toast(`Saved preferences for ${n}`);
                }}
              >
                {copilotTokenSaving || opencodeGoSaving || providerApiKeySaving
                  ? "Saving..."
                  : "Save"}
              </Button>
            </ModalFooter>
          </>
        ) : null}
      </Modal>
    </ScreenScroll>
  );
}
