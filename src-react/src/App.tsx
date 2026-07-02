import { useCallback, useEffect, useRef, useState } from "react";
import { AppShell } from "@/components/shell/AppShell";
import { ProviderPage } from "@/components/provider/ProviderPage";
import { useTraySync } from "@/components/provider/useTraySync";
import { ProjectPage } from "@/components/project/ProjectPage";
import { SkillPage } from "@/components/skill/SkillPage";
import { PromptPage } from "@/components/prompt/PromptPage";
import { SessionPage } from "@/components/session/SessionPage";
import { SyncPage } from "@/components/sync/SyncPage";
import { SettingsPage } from "@/components/settings/SettingsPage";
import {
  detectDesktopHealth,
  type DesktopHealthState,
} from "@/lib/runtime";
import { NavContext, type Nav, type View } from "@/lib/nav";

export default function App() {
  const [view, setView] = useState<View>("provider");
  const [desktopHealth, setDesktopHealth] = useState<DesktopHealthState>({
    status: "unknown",
  });
  const desktopProbeStarted = useRef(false);
  // Deep-link target for Project detail (set when navigating from Session).
  const [projectId, setProjectId] = useState<string | undefined>(undefined);

  // Keep the Windows-taskbar tray in sync app-wide, regardless of current page.
  useTraySync();

  const go = useCallback<Nav["go"]>((v, opts) => {
    setView(v);
    setProjectId(opts?.projectId);
  }, []);

  useEffect(() => {
    if (desktopProbeStarted.current) {
      return;
    }

    desktopProbeStarted.current = true;
    let active = true;

    void detectDesktopHealth().then((health) => {
      if (active) {
        setDesktopHealth(health);
      }
    });

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (import.meta.env.DEV && desktopHealth.status === "connected") {
      console.debug(
        `Agent Nexus desktop host connected: ${desktopHealth.appName} ${desktopHealth.appVersion}`,
      );
    }
  }, [desktopHealth]);

  return (
    <NavContext.Provider value={{ view, go }}>
      <AppShell>
        {view === "provider" && <ProviderPage />}
        {view === "project" && <ProjectPage initialProjectId={projectId} />}
        {view === "skill" && <SkillPage />}
        {view === "prompt" && <PromptPage />}
        {view === "session" && <SessionPage />}
        {view === "sync" && <SyncPage />}
        {view === "settings" && <SettingsPage />}
      </AppShell>
    </NavContext.Provider>
  );
}
