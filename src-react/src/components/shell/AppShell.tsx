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

function MenuBar() {
  const { view, go } = useNav();
  const onSettings = view === "settings";
  return (
    <div className="flex flex-none items-center gap-2 border-b border-nexus-border2 bg-nexus-bg px-[14px] py-[9px]">
      <div className="flex items-center gap-1">
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
      <span className="ml-auto inline-flex items-center gap-[7px] rounded-full border border-nexus-border2 bg-nexus-card px-[11px] py-[5px] text-[11.5px] text-[#a99a89]">
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
  );
}

/** App content shell: menu bar + page content slot.
 *  Pages own their own scroll/padding inside the flex-1 region. */
export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="fixed inset-0 flex flex-col overflow-hidden bg-nexus-bg">
      <MenuBar />
      {children}
    </div>
  );
}
