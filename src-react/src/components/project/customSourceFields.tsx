import type { ReactNode } from "react";
import { AGENTS } from "@/config/agents";

/** Per-agent glob an Extra Prompt File must match (e.g. `AGENTS.md` → `AGENTS*.md`). */
export const PROMPT_FILE_GLOBS = AGENTS.filter((agent) => agent.projectPromptFile).map(
  (agent) => {
    const file = agent.projectPromptFile as string;
    const stem = file.replace(/\.md$/i, "");
    return { agent: agent.name, glob: `${stem}*.md`, re: new RegExp(`^${stem}.*\\.md$`, "i") };
  },
);

function basename(file: string): string {
  return file.trim().replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? "";
}

/** True when the basename of `file` matches an Agent prompt-file glob. */
export function matchesPromptGlob(file: string): boolean {
  const base = basename(file);
  return PROMPT_FILE_GLOBS.some((g) => g.re.test(base));
}

/** Badge marking a custom skills dir as living inside the repo or at an absolute /
 *  home path outside it. Shared by the per-Project and default-source editors. */
export function renderSkillDirBadge(dir: string): ReactNode {
  const external = /^(~|\/|[A-Za-z]:[\\/])/.test(dir);
  return (
    <span
      className="flex-none rounded-[5px] px-[7px] py-0.5 text-[9.5px] font-bold uppercase tracking-[.04em]"
      style={{
        color: external ? "#9a6f0a" : "#5f7a3e",
        background: external ? "#f7eccb" : "#e9eed8",
      }}
    >
      {external ? "External" : "In repo"}
    </span>
  );
}

/** Badge naming the Source Agent that owns an extra prompt file's glob. */
export function renderPromptFileBadge(file: string): ReactNode {
  const owner = PROMPT_FILE_GLOBS.find((g) => g.re.test(basename(file)))?.agent;
  return owner ? (
    <span className="flex-none rounded-[5px] bg-[#e9eed8] px-[7px] py-0.5 text-[9.5px] font-bold uppercase tracking-[.04em] text-[#5f7a3e]">
      {owner}
    </span>
  ) : null;
}
