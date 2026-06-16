import type { ReactNode } from "react";
import { Settings } from "lucide-react";
import { IconButton } from "@/components/ui/button";
import { useNav, type View } from "@/lib/nav";
import { cn } from "@/lib/utils";

const TABS: { key: View; label: string }[] = [
  { key: "provider", label: "Provider" },
  { key: "project", label: "Project" },
  { key: "skill", label: "Skill" },
  { key: "prompt", label: "Prompt" },
  { key: "session", label: "Session" },
  { key: "sync", label: "Sync" },
];

function TitleBar() {
  const { view, go } = useNav();
  const onSettings = view === "settings";
  return (
    <div className="relative flex h-10 flex-none items-center border-b border-[#e0d4c2] bg-nexus-titlebar px-[14px]">
      <div className="flex items-center gap-[7px]">
        <span className="h-[11px] w-[11px] rounded-full bg-[#ddd0bf]" />
        <span className="h-[11px] w-[11px] rounded-full bg-[#ddd0bf]" />
        <span className="h-[11px] w-[11px] rounded-full bg-[#ddd0bf]" />
      </div>
      <div className="absolute left-1/2 flex -translate-x-1/2 items-center gap-2 text-[12.5px] font-bold text-[#5a4d42]">
        <span className="inline-block h-[14px] w-[14px] rounded-[5px] bg-nexus-accent" />
        Agent Nexus
      </div>
      <div className="ml-auto flex items-center gap-2 text-[11.5px] text-[#a99a89]">
        <span className="inline-flex items-center gap-[7px] rounded-full border border-nexus-border2 bg-nexus-card px-[11px] py-[5px]">
          Search assets
          <span className="rounded-[4px] border border-nexus-border2 px-[6px] py-[2px] font-mono text-[10px] text-[#b8a890]">
            ⌘K
          </span>
        </span>
        <IconButton
          dim={30}
          variant="card"
          title="Settings"
          onClick={() => go("settings")}
          className={cn(
            onSettings && "border-[#e0d4c2] bg-nexus-panel text-nexus-accent",
          )}
        >
          <Settings size={14} />
        </IconButton>
      </div>
    </div>
  );
}

function TabNav() {
  const { view, go } = useNav();
  return (
    <div className="flex flex-none items-center gap-1 border-b border-nexus-border2 bg-nexus-bg px-[14px] py-[9px]">
      {TABS.map((t) => {
        const active = t.key === view;
        return (
          <div
            key={t.key}
            onClick={() => go(t.key)}
            className={cn(
              "cursor-pointer rounded-full border px-[15px] py-[7px] text-[13px] transition-colors",
              active
                ? "border-nexus-border2 bg-nexus-card font-bold text-nexus-accent shadow-[0_1px_2px_rgba(50,40,25,.05)]"
                : "border-transparent font-semibold text-[#9a8f80] hover:text-nexus-accent",
            )}
          >
            {t.label}
          </div>
        );
      })}
    </div>
  );
}

/** Full-window chrome: 40px title bar + tab bar + page content slot.
 *  Pages own their own scroll/padding inside the flex-1 region. */
export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="fixed inset-0 flex flex-col overflow-hidden bg-nexus-bg">
      <TitleBar />
      <TabNav />
      {children}
    </div>
  );
}
