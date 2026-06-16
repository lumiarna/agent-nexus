import { useCallback, useState } from "react";
import { AppShell } from "@/components/shell/AppShell";
import { ScreenScroll } from "@/components/shell/screen";
import { ProviderPage } from "@/components/provider/ProviderPage";
import { NavContext, type Nav, type View } from "@/lib/nav";

// Temporary stub for pages built after the Provider milestone checkpoint.
function Placeholder({ name }: { name: string }) {
  return (
    <ScreenScroll>
      <h1 className="m-0 text-[23px] font-extrabold tracking-[-.02em] text-nexus-ink">
        {name}
      </h1>
      <p className="mt-1.5 text-[13px] text-[#9a8f80]">Coming up next in this build.</p>
    </ScreenScroll>
  );
}

export default function App() {
  const [view, setView] = useState<View>("provider");
  // Deep-link target for Project detail (set when navigating from Session).
  const [, setProjectId] = useState<string | undefined>(undefined);

  const go = useCallback<Nav["go"]>((v, opts) => {
    setView(v);
    setProjectId(opts?.projectId);
  }, []);

  return (
    <NavContext.Provider value={{ view, go }}>
      <AppShell>
        {view === "provider" && <ProviderPage />}
        {view === "project" && <Placeholder name="Project" />}
        {view === "skill" && <Placeholder name="Skill" />}
        {view === "prompt" && <Placeholder name="Prompt" />}
        {view === "session" && <Placeholder name="Session" />}
        {view === "sync" && <Placeholder name="Sync" />}
        {view === "settings" && <Placeholder name="Settings" />}
      </AppShell>
    </NavContext.Provider>
  );
}
