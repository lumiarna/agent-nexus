import { useEffect, useRef, useState } from "react";
import { Settings } from "lucide-react";
import { toast } from "sonner";
import { Button, IconButton } from "@/components/ui/button";
import { Dot, Input } from "@/components/ui/primitives";
import { Modal, ModalFooter, ModalHeader } from "@/components/ui/modal";
import { Segmented } from "@/components/ui/segmented";
import { Toggle } from "@/components/ui/toggle";
import { ScreenScroll } from "@/components/shell/screen";
import { nexus } from "@/lib/mock";
import { quotaColor, statusInfo, type ProviderUiStatus } from "@/lib/tokens";
import { cn } from "@/lib/utils";
import type { Provider, TrayMetric } from "@/types";

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

export function ProviderPage() {
  const [providers] = useState<Provider[]>(() => nexus.providers());
  const [order, setOrder] = useState<string[]>(() => providers.map((p) => p.id));
  const [cardVisible, setCardVisible] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(providers.map((p) => [p.id, !p.hiddenCard])),
  );
  const [trayVisible, setTrayVisible] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(providers.map((p) => [p.id, p.status === "available"])),
  );
  const [configId, setConfigId] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState<Record<string, boolean>>({});
  const [dragId, setDragId] = useState<string | null>(null);
  const [trayMetric, setTrayMetric] = useState<TrayMetric>(() => nexus.settings().trayMetric);

  const timers = useRef<Record<string, number>>({});
  useEffect(
    () => () => Object.values(timers.current).forEach((t) => window.clearTimeout(t)),
    [],
  );

  function refreshProvider(id: string) {
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

  function reorder(fromId: string | null, toId: string) {
    if (!fromId || fromId === toId) return;
    setOrder((o) => {
      const a = [...o];
      const fi = a.indexOf(fromId);
      const ti = a.indexOf(toId);
      if (fi < 0 || ti < 0) return o;
      a.splice(fi, 1);
      a.splice(ti, 0, fromId);
      return a;
    });
  }

  const byId = Object.fromEntries(providers.map((p) => [p.id, p]));
  const ordered = order.map((id) => byId[id]).filter(Boolean) as Provider[];
  const visible = ordered.filter((p) => cardVisible[p.id] !== false);
  const hidden = ordered.filter((p) => cardVisible[p.id] === false);
  const cfg = configId ? providers.find((p) => p.id === configId) ?? null : null;

  return (
    <ScreenScroll>
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="m-0 text-[23px] font-extrabold tracking-[-.02em] text-nexus-ink">
            Provider
          </h1>
          <p className="mt-1.5 text-[13px] text-[#9a8f80]">
            Global quota &amp; credential visibility · {visible.length} of{" "}
            {providers.length} shown · drag cards to reorder
          </p>
        </div>
        <Button
          variant="secondary"
          onClick={() => {
            providers.forEach((p) => refreshProvider(p.id));
            toast("Refreshing all providers…");
          }}
        >
          Refresh all
        </Button>
      </div>

      <div
        className="mt-[22px] grid gap-4"
        style={{ gridTemplateColumns: "repeat(auto-fill,minmax(300px,1fr))" }}
      >
        {visible.map((p) => {
          const loading = !!refreshing[p.id];
          const st: ProviderUiStatus = loading ? "loading" : p.status;
          const si = statusInfo(st);
          const showQuota = st === "available" && !!p.windows;
          const hasMessage =
            !loading &&
            (p.status === "expired" || p.status === "nocreds" || p.status === "failed");
          const mm = MSG[p.status] ?? { title: "", body: "" };
          const body = p.status === "failed" ? p.error ?? mm.body : mm.body;
          const dragging = dragId === p.id;
          return (
            <div
              key={p.id}
              draggable
              onDragStart={(e) => {
                setDragId(p.id);
                e.dataTransfer.effectAllowed = "move";
                try {
                  e.dataTransfer.setData("text/plain", p.id);
                } catch {
                  /* some browsers disallow setData here */
                }
              }}
              onDragOver={(e) => {
                e.preventDefault();
                e.dataTransfer.dropEffect = "move";
              }}
              onDrop={(e) => {
                e.preventDefault();
                reorder(dragId, p.id);
                setDragId(null);
              }}
              onDragEnd={() => setDragId(null)}
              className={cn(
                "flex flex-col rounded-[18px] border bg-nexus-card p-[18px] transition-[box-shadow,opacity]",
                dragging
                  ? "border-nexus-accent opacity-60 shadow-[0_8px_28px_rgba(50,40,25,.16)]"
                  : "border-nexus-border shadow-[0_1px_14px_rgba(50,40,25,.05)]",
              )}
            >
              <div className="flex items-start justify-between gap-2.5">
                <div className="flex min-w-0 items-start gap-[9px]">
                  <span
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
                  <div className="mt-[18px] flex items-baseline gap-[7px]">
                    <span
                      className="text-[30px] font-extrabold leading-none tracking-[-.03em]"
                      style={{ color: quotaColor(p.primary ?? 0) }}
                    >
                      {p.primary != null ? `${p.primary}%` : ""}
                    </span>
                    <span className="text-[12px] text-[#b3a999]">peak window used</span>
                  </div>
                  <div className="mt-[15px] flex flex-col gap-[13px]">
                    {(p.windows ?? []).map((w) => (
                      <div key={w.label}>
                        <div className="mb-1.5 flex justify-between text-[12px]">
                          <span className="text-[#6a6055]">{w.label}</span>
                          <span className="font-bold" style={{ color: quotaColor(w.used) }}>
                            {w.used}%
                          </span>
                        </div>
                        <div className="h-2 overflow-hidden rounded-full bg-[#ece2d6]">
                          <div
                            className="h-full rounded-full"
                            style={{ width: `${w.used}%`, background: quotaColor(w.used) }}
                          />
                        </div>
                        <div className="mt-1.5 text-[11px] text-[#b3a999]">{w.reset}</div>
                      </div>
                    ))}
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
            </div>
          );
        })}
      </div>

      {hidden.length > 0 ? (
        <div className="mt-5 flex flex-wrap items-center gap-3 rounded-[14px] border border-dashed border-[#ddccb6] bg-nexus-panel px-[18px] py-[14px]">
          <span className="text-[12px] font-bold text-[#7a6f60]">Hidden on this page</span>
          {hidden.map((p) => (
            <div
              key={p.id}
              onClick={() => {
                setCardVisible((cv) => ({ ...cv, [p.id]: true }));
                toast(`${p.name} shown on Provider page`);
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
                {cfg.needsParams ? (
                  <div className="flex flex-col gap-[13px]">
                    {[
                      {
                        label: "Workspace ID",
                        placeholder: "ws_xxxxxxxxxxxx",
                        hint: "Required to query the workspace quota endpoint",
                      },
                      {
                        label: "Auth Cookie",
                        placeholder: "session=…",
                        hint: "Treated only as a quota-observation connection param — not a login",
                      },
                    ].map((f) => (
                      <label key={f.label} className="block">
                        <div className="mb-1.5 text-[12px] font-semibold text-[#6a6055]">
                          {f.label}
                        </div>
                        <Input className="font-mono" placeholder={f.placeholder} defaultValue="" />
                        <div className="mt-[5px] text-[11px] text-[#b3a999]">{f.hint}</div>
                      </label>
                    ))}
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
                    onClick={() =>
                      setCardVisible((cv) => ({ ...cv, [cfg.id]: !(cv[cfg.id] !== false) }))
                    }
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
                onClick={() => {
                  const n = cfg.name;
                  setConfigId(null);
                  toast(`Saved preferences for ${n}`);
                }}
              >
                Save
              </Button>
            </ModalFooter>
          </>
        ) : null}
      </Modal>
    </ScreenScroll>
  );
}
