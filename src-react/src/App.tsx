import { useCallback, useState } from "react";
import { AppShell } from "@/components/shell/AppShell";
import { ProviderPage } from "@/components/provider/ProviderPage";
import { ProjectPage } from "@/components/project/ProjectPage";
import { SkillPage } from "@/components/skill/SkillPage";
import { PromptPage } from "@/components/prompt/PromptPage";
import { SessionPage } from "@/components/session/SessionPage";
import { SyncPage } from "@/components/sync/SyncPage";
import { SettingsPage } from "@/components/settings/SettingsPage";
import { NavContext, type Nav, type View } from "@/lib/nav";

export default function App() {
  const [view, setView] = useState<View>("provider");
  // Deep-link target for Project detail (set when navigating from Session).
  const [projectId, setProjectId] = useState<string | undefined>(undefined);

  const go = useCallback<Nav["go"]>((v, opts) => {
    setView(v);
    setProjectId(opts?.projectId);
  }, []);

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
