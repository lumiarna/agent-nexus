import { useState } from "react";
import { toast } from "sonner";
import {
  AgentMatrixCells,
  MatrixLegend,
  SourceBadge,
} from "@/components/ui/agent-icon";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/primitives";
import { Segmented } from "@/components/ui/segmented";
import { SkillRow } from "@/components/skill/SkillRow";
import { ScreenScroll } from "@/components/shell/screen";
import { AGENTS } from "@/config/agents";
import { promptsApi } from "@/lib/api/prompts";
import { skillsApi } from "@/lib/api/skills";
import { useNav } from "@/lib/nav";
import { useSetProjectCustomSkillsDirsMutation, useSetProjectExtraPromptFilesMutation, useSetProjectSessionsDirMutation } from "@/lib/query/projects";
import { usePromptsQuery, useSetPromptTargetMutation } from "@/lib/query/prompts";
import {
  useCloudSessionsQuery,
  useLocalSessionsQuery,
} from "@/lib/query/sessions";
import {
  useSetSkillDisabledMutation,
  useSetSkillTargetMutation,
  useSkillsQuery,
} from "@/lib/query/skills";
import { palette, srcAgentOf, targetAgentsOf } from "@/lib/tokens";
import type { AgentName, Project, Prompt, Skill } from "@/types";
import {
  matchesPromptGlob,
  renderPromptFileBadge,
  renderSkillDirBadge,
} from "./customSourceFields";
import { getErrorMessage } from "./getErrorMessage";
import { DEFAULT_SESSIONS_DIR } from "./projectShared";
import { SingleValueConfigModal } from "./SingleValueConfigModal";
import { StringListConfigModal } from "./StringListConfigModal";

type DetailSource = "local" | "cloud";

/** Project prompts collapse to the repo-root files owned by prompt-capable
 *  agents — Generic Agent (AGENTS.md) and Claude Code (CLAUDE.md). */
const PROJECT_PROMPT_AGENTS: AgentName[] = AGENTS.filter(
  (agent) => agent.projectPromptFile,
).map((agent) => agent.name);

interface ProjectDetailViewProps {
  project: Project;
  desktop: boolean;
  onBack: () => void;
}

/**
 * Presentational detail screen for one Project: skill / prompt / session /
 * sync cards plus the three custom-source modals. Owns its own detail-scoped
 * state (selected source, modal open flags) and reads/writes through the
 * existing query hooks, so ProjectPage stays a thin router.
 */
export function ProjectDetailView({ project: dp, desktop, onBack }: ProjectDetailViewProps) {
  const { go } = useNav();
  const skillsQuery = useSkillsQuery();
  const promptsQuery = usePromptsQuery();
  const localSessionsQuery = useLocalSessionsQuery();
  const cloudSessionsQuery = useCloudSessionsQuery();
  const setSkillTarget = useSetSkillTargetMutation();
  const setSkillDisabled = useSetSkillDisabledMutation();
  const setPromptTarget = useSetPromptTargetMutation();
  const setCustomSkillsDirs = useSetProjectCustomSkillsDirsMutation();
  const setExtraPromptFiles = useSetProjectExtraPromptFilesMutation();
  const setSessionsDir = useSetProjectSessionsDirMutation();

  const [detailSource, setDetailSource] = useState<DetailSource>("local");
  const [customDirsOpen, setCustomDirsOpen] = useState(false);
  const [extraFilesOpen, setExtraFilesOpen] = useState(false);
  const [sessionDirOpen, setSessionDirOpen] = useState(false);

  const skills = skillsQuery.data ?? [];
  const prompts = promptsQuery.data ?? [];
  const dpSkills = skills.filter((k) => k.scope === "project" && k.projectId === dp.id);
  const dpPrompts = prompts.filter((p) => p.scope === "project" && p.projectId === dp.id);
  const detailSessionsQuery = detailSource === "local" ? localSessionsQuery : cloudSessionsQuery;
  const detailSessions = detailSessionsQuery.data ?? [];
  const dpSessions = detailSessions.filter(
    (session) =>
      session.project === dp.id &&
      (session.source === detailSource || session.source === "both"),
  );
  const skillError = skillsQuery.error ? getErrorMessage(skillsQuery.error) : null;
  const promptError = promptsQuery.error ? getErrorMessage(promptsQuery.error) : null;
  const detailSessionError = detailSessionsQuery.error
    ? getErrorMessage(detailSessionsQuery.error)
    : null;

  async function toggleCell(skill: Skill, agent: AgentName) {
    if (skill.cells[agent] === "source") return;
    if (!desktop) {
      toast("Desktop runtime required for changing skill targets");
      return;
    }

    try {
      await setSkillTarget.mutateAsync({
        skillId: skill.id,
        agent,
        enabled: skill.cells[agent] !== "target",
      });
      toast(skill.cells[agent] === "target" ? "Target removed" : "Target linked");
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function propagateGlobal(skill: Skill, entryAgent: AgentName) {
    if (!desktop) {
      toast("Desktop runtime required for propagating skills");
      return;
    }

    try {
      await setSkillTarget.mutateAsync({ skillId: skill.id, agent: entryAgent, enabled: true });
      toast(`Propagated to Global · ${entryAgent}`);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function unpropagateGlobal(skill: Skill) {
    if (!desktop) {
      toast("Desktop runtime required for changing skill placements");
      return;
    }

    try {
      for (const agent of targetAgentsOf(skill.cells)) {
        await setSkillTarget.mutateAsync({ skillId: skill.id, agent, enabled: false });
      }
      toast("Removed from Global");
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function toggleDmi(skill: Skill) {
    if (!desktop) {
      toast("Desktop runtime required for changing skill settings");
      return;
    }

    try {
      await setSkillDisabled.mutateAsync({ id: skill.id, disabled: !skill.disabled });
      toast(!skill.disabled ? "Model invocation disabled" : "Model invocation enabled");
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function togglePromptCell(prompt: Prompt, agent: AgentName) {
    if (prompt.cells[agent] === "source") return;
    if (!desktop) {
      toast("Desktop runtime required for changing prompt targets");
      return;
    }

    try {
      await setPromptTarget.mutateAsync({
        promptId: prompt.id,
        agent,
        enabled: prompt.cells[agent] !== "target",
      });
      toast(prompt.cells[agent] === "target" ? "Target removed" : "Target linked");
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function openPromptSource(prompt: Prompt) {
    if (!desktop) {
      toast("Desktop runtime required for opening source files");
      return;
    }

    try {
      await promptsApi.openSource(prompt.id);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function revealPromptPath(prompt: Prompt) {
    if (!desktop) {
      toast("Desktop runtime required for revealing files");
      return;
    }

    try {
      await promptsApi.revealPath(prompt.id);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function openSkillSource(skill: Skill) {
    if (!desktop) {
      toast("Desktop runtime required for opening source files");
      return;
    }

    try {
      await skillsApi.openSource(skill.id);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  async function revealSkillPath(skill: Skill) {
    if (!desktop) {
      toast("Desktop runtime required for revealing files");
      return;
    }

    try {
      await skillsApi.revealPath(skill.id);
    } catch (error) {
      toast(getErrorMessage(error));
    }
  }

  return (
    <ScreenScroll>
      <button
        onClick={onBack}
        className="mb-3.5 inline-flex items-center gap-1.5 text-[12px] text-[#9a8f80] hover:text-nexus-accent"
      >
        ← Project
      </button>

      <Card className="p-[22px]">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <h1 className="m-0 text-[21px] font-extrabold tracking-[-.02em] text-nexus-ink">
              {dp.name}
            </h1>
            <div className="mt-2.5 flex flex-wrap gap-x-6 gap-y-2 text-[12px]">
              <div>
                <span className="text-[#b3a999]">Repo&nbsp;&nbsp;</span>
                <span className="font-mono text-[#6a6055]">{dp.path}</span>
              </div>
              <div>
                <span className="text-[#b3a999]">Key&nbsp;&nbsp;</span>
                <span className="font-mono text-[#6a6055]">{dp.key}</span>
                <span className="ml-1.5 text-[10px] text-[#bca37a]">folder name</span>
              </div>
              <div>
                <span className="text-[#b3a999]">Session dir&nbsp;&nbsp;</span>
                <span className="font-mono text-[#6a6055]">{dp.sessionsDir}</span>
              </div>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button variant="secondary" size="sm" className="px-3.5" onClick={() => go("sync")}>
              Open in Sync
            </Button>
          </div>
        </div>
      </Card>

      {/* Skill table */}
      <Card className="mt-4 overflow-hidden">
        <div className="flex items-center justify-between gap-2.5 px-5 pb-1 pt-4">
          <span className="text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
            Skill
          </span>
          <div className="flex items-center gap-3">
            <button
              onClick={() => setCustomDirsOpen(true)}
              className="text-[11px] font-semibold text-nexus-accent hover:underline"
            >
              Custom skills dirs
              {dp.customSkillsDirs && dp.customSkillsDirs.length > 0
                ? ` · ${dp.customSkillsDirs.length}`
                : ""}
            </button>
            <span className="text-[11px] text-[#b3a999]">
              project scope · {dpSkills.length} {dpSkills.length === 1 ? "skill" : "skills"}
            </span>
          </div>
        </div>
        <div
          className="grid items-center gap-4 px-5 py-2.5 text-[10.5px] font-bold uppercase tracking-[.05em] text-[#b3a999]"
          style={{ gridTemplateColumns: "1fr 196px 116px 132px" }}
        >
          <div>Skill</div>
          <div className="text-center">Distribution</div>
          <div className="text-center">Disable invoke</div>
          <div className="text-right">Source file</div>
        </div>
        {skillsQuery.isLoading && dpSkills.length === 0 ? (
          <div className="mx-5 mb-5 rounded-[12px] border border-dashed border-nexus-border2 bg-nexus-sand p-[18px] text-center text-[12px] text-[#b3a999]">
            Loading project skills...
          </div>
        ) : skillError ? (
          <div className="mx-5 mb-5 rounded-[12px] border border-[#ecd0c6] bg-[#f8ebe6] p-[18px] text-center text-[12px] text-nexus-crit">
            {skillError}
          </div>
        ) : dpSkills.length > 0 ? (
          dpSkills.map((k) => (
            <SkillRow
              key={k.id}
              skill={k}
              mode="project"
              projectName={dp.name}
              onToggleCell={(a) => void toggleCell(k, a)}
              onToggleDmi={() => void toggleDmi(k)}
              onPropagateGlobal={(entry) => void propagateGlobal(k, entry)}
              onUnpropagateGlobal={() => void unpropagateGlobal(k)}
              onOpen={() => void openSkillSource(k)}
              onReveal={() => void revealSkillPath(k)}
            />
          ))
        ) : (
          <div className="mx-5 mb-5 rounded-[12px] border border-dashed border-nexus-border2 bg-nexus-sand p-[18px] text-center text-[12px] text-[#b3a999]">
            No project skills recorded for this repository.
          </div>
        )}
        <div className="mx-5 mb-[18px] flex items-center justify-between gap-3">
          <button onClick={() => go("skill")} className="inline-flex text-[12px] font-semibold text-nexus-accent hover:underline">
            Open in Skill →
          </button>
          <MatrixLegend />
        </div>
      </Card>

      {/* Prompt table */}
      <Card className="mt-4 overflow-hidden">
        <div className="flex items-center justify-between gap-2.5 px-5 pb-1 pt-4">
          <span className="text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
            Prompt
          </span>
          <div className="flex items-center gap-3">
            <button
              onClick={() => setExtraFilesOpen(true)}
              className="text-[11px] font-semibold text-nexus-accent hover:underline"
            >
              Custom prompt files
              {dp.extraPromptFiles && dp.extraPromptFiles.length > 0
                ? ` · ${dp.extraPromptFiles.length}`
                : ""}
            </button>
            <span className="text-[11px] text-[#b3a999]">
              project scope · {dpPrompts.length} {dpPrompts.length === 1 ? "prompt" : "prompts"}
            </span>
          </div>
        </div>
        <div
          className="grid items-center gap-4 px-5 py-2.5 text-[10.5px] font-bold uppercase tracking-[.05em] text-[#b3a999]"
          style={{ gridTemplateColumns: "1fr 196px 132px" }}
        >
          <div>Prompt</div>
          <div className="text-center">Distribution</div>
          <div className="text-right">Source file</div>
        </div>
        {promptsQuery.isLoading && dpPrompts.length === 0 ? (
          <div className="mx-5 mb-5 rounded-[12px] border border-dashed border-nexus-border2 bg-nexus-sand p-[18px] text-center text-[12px] text-[#b3a999]">
            Loading project prompts...
          </div>
        ) : promptError ? (
          <div className="mx-5 mb-5 rounded-[12px] border border-[#ecd0c6] bg-[#f8ebe6] p-[18px] text-center text-[12px] text-nexus-crit">
            {promptError}
          </div>
        ) : dpPrompts.length > 0 ? (
          dpPrompts.map((p) => (
            <div
              key={p.id}
              className="grid items-center gap-4 border-t border-[#f3eee5] px-5 py-[14px]"
              style={{ gridTemplateColumns: "1fr 196px 132px" }}
            >
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-[14px] font-bold text-nexus-ink">{p.name}</span>
                  <SourceBadge agent={srcAgentOf(p.cells)} />
                </div>
                <div className="mt-[3px] overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11.5px] text-[#a99a89]">
                  {p.path}
                </div>
              </div>
              <AgentMatrixCells
                cells={p.cells}
                agents={PROJECT_PROMPT_AGENTS}
                onToggle={(a) => void togglePromptCell(p, a)}
              />
              <div className="flex flex-col items-end gap-[5px]">
                <span
                  onClick={() => void openPromptSource(p)}
                  className="cursor-pointer whitespace-nowrap text-[11.5px] font-semibold text-nexus-accent hover:underline"
                >
                  Open source
                </span>
                <span
                  onClick={() => void revealPromptPath(p)}
                  className="cursor-pointer whitespace-nowrap text-[11.5px] font-semibold text-[#a99a89] hover:underline"
                >
                  Reveal path
                </span>
              </div>
            </div>
          ))
        ) : (
          <div className="mx-5 mb-5 rounded-[12px] border border-dashed border-nexus-border2 bg-nexus-sand p-[18px] text-center text-[12px] text-[#b3a999]">
            No project prompts discovered. Add AGENTS.md / CLAUDE.md at the repo root, or register extra prompt files.
          </div>
        )}
        <div className="mx-5 mb-[18px] flex items-center justify-between gap-3">
          <button onClick={() => go("prompt")} className="inline-flex text-[12px] font-semibold text-nexus-accent hover:underline">
            Open in Prompt →
          </button>
          <MatrixLegend />
        </div>
      </Card>

      {/* Session panel */}
      <Card className="mt-4 p-5">
        <div className="mb-3.5 flex items-center justify-between gap-3">
          <span className="text-[11px] font-bold uppercase tracking-[.06em] text-nexus-accent">
            Session
          </span>
          <div className="flex items-center gap-3">
            <button
              onClick={() => setSessionDirOpen(true)}
              className="text-[11px] font-semibold text-nexus-accent hover:underline"
            >
              Configure session dir
              {dp.sessionsDir && dp.sessionsDir !== DEFAULT_SESSIONS_DIR
                ? ` · ${dp.sessionsDir}`
                : ""}
            </button>
            <Segmented<DetailSource>
              options={[
                { value: "local", label: "Local" },
                { value: "cloud", label: "Cloud" },
              ]}
              value={detailSource}
              onChange={setDetailSource}
              size="md"
            />
          </div>
        </div>
        {detailSessionsQuery.isLoading && dpSessions.length === 0 ? (
          <div className="rounded-[12px] border border-dashed border-nexus-border2 bg-nexus-sand p-[18px] text-center text-[12px] text-[#b3a999]">
            Loading {detailSource} sessions...
          </div>
        ) : detailSessionError ? (
          <div className="rounded-[12px] border border-[#ecd0c6] bg-[#f8ebe6] p-[18px] text-center text-[12px] text-nexus-crit">
            {detailSessionError}
          </div>
        ) : dpSessions.length > 0 ? (
          <div className="flex flex-col gap-0.5">
            {dpSessions.map((se) => (
              <div
                key={se.id}
                className="flex items-center justify-between gap-3 rounded-[10px] px-[11px] py-2.5 hover:bg-nexus-sand"
              >
                <div className="min-w-0">
                  <div className="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[12.5px] font-bold text-nexus-body">
                    {se.title}
                  </div>
                  <div className="overflow-hidden text-ellipsis whitespace-nowrap text-[11px] text-[#b3a999]">
                    {se.excerpt}
                  </div>
                </div>
                <div className="flex-none whitespace-nowrap text-[11px] text-[#c3b9a8]">
                  {se.updated}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="rounded-[12px] border border-dashed border-nexus-border2 bg-nexus-sand p-[18px] text-center text-[12px] text-[#b3a999]">
            {detailSource === "cloud"
              ? "No archived sessions in the Cloud for this project yet."
              : "No local sessions in the session directory."}
          </div>
        )}
        <button onClick={() => go("session")} className="mt-3 inline-flex text-[12px] font-semibold text-nexus-accent hover:underline">
          Open in Session →
        </button>
      </Card>

      {/* Custom skills dirs modal */}
      <StringListConfigModal
        open={customDirsOpen}
        onClose={() => setCustomDirsOpen(false)}
        title="Project custom skills dirs"
        subtitle="Extra scan sources alongside the fixed Agent project skills dirs"
        items={dp.customSkillsDirs ?? []}
        onAdd={(dirs) => setCustomSkillsDirs.mutateAsync({ projectId: dp.id, dirs })}
        onRemove={(dirs) => setCustomSkillsDirs.mutateAsync({ projectId: dp.id, dirs })}
        placeholder="skills  ·  .nexus/skills  ·  /abs/path/to/skills"
        addLabel="Add dir"
        initialInput="skills"
        busy={setCustomSkillsDirs.isPending}
        messages={{
          added: (dir) => `Added custom skills dir · ${dir}`,
          removed: (dir) => `Removed custom skills dir · ${dir}`,
          duplicate: "Directory already added",
        }}
        emptyHint="No custom skills dirs. Relative paths resolve against the Project root."
        renderBadge={renderSkillDirBadge}
        help={
          <>
            Each dir is scanned for real Skill folders (containing{" "}
            <span className="font-mono">SKILL.md</span>) as Project custom sources — they show no
            Agent Matrix and can be propagated to Global. A dir that resolves to a fixed Agent
            project skills dir is rejected. Removing a dir drops its custom Skills on the next
            scan; managed Global placements fall back to none.
          </>
        }
      />

      {/* Extra prompt files modal */}
      <StringListConfigModal
        open={extraFilesOpen}
        onClose={() => setExtraFilesOpen(false)}
        title="Project extra prompt files"
        subtitle="Extra Prompt files scanned alongside the primary AGENTS.md / CLAUDE.md"
        items={dp.extraPromptFiles ?? []}
        onAdd={(files) => setExtraPromptFiles.mutateAsync({ projectId: dp.id, files })}
        onRemove={(files) => setExtraPromptFiles.mutateAsync({ projectId: dp.id, files })}
        validate={(file) =>
          matchesPromptGlob(file) ? null : "File must match AGENTS*.md or CLAUDE*.md"
        }
        placeholder="AGENTS.local.md  ·  docs/CLAUDE.md"
        addLabel="Add file"
        busy={setExtraPromptFiles.isPending}
        messages={{
          added: (file) => `Added extra prompt file · ${file}`,
          removed: (file) => `Removed extra prompt file · ${file}`,
          duplicate: "File already added",
        }}
        emptyHint="No extra prompt files. Paths resolve against the Project root."
        renderBadge={renderPromptFileBadge}
        help={
          <>
            Each file&apos;s name must match an Agent prompt-file glob —{" "}
            <span className="font-mono">AGENTS*.md</span> (Generic Agent) or{" "}
            <span className="font-mono">CLAUDE*.md</span> (Claude Code). The matching Agent
            becomes the Source Agent; files that match neither are rejected. This widens the
            Prompt scan inside an existing Agent namespace — it does not create a new source.
          </>
        }
      />

      {/* Session dir modal */}
      <SingleValueConfigModal
        open={sessionDirOpen}
        onClose={() => setSessionDirOpen(false)}
        title="Configure session dir"
        subtitle="Override the single Session Directory for this Project"
        label="Session directory"
        initialValue={
          dp.sessionsDir && dp.sessionsDir !== DEFAULT_SESSIONS_DIR ? dp.sessionsDir : ""
        }
        placeholder={DEFAULT_SESSIONS_DIR}
        onSubmit={async (dir) => {
          const project = await setSessionsDir.mutateAsync({ projectId: dp.id, dir });
          return project.sessionsDir;
        }}
        busy={setSessionsDir.isPending}
        messages={{
          set: (canonical) => `Session dir set · ${canonical}`,
          cleared: "Session dir restored to default",
        }}
        help={
          <>
            A Project always has exactly one Session Directory — this is a deliberate constraint,
            not an MVP limit. Relative paths resolve against the Project root. Leave empty to
            restore the default <span className="font-mono">{DEFAULT_SESSIONS_DIR}</span>.
          </>
        }
      />
    </ScreenScroll>
  );
}
