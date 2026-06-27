import type { GitBaseFolder, Project, ScanResult } from "@/types";
import { invokeCommand } from "@/lib/api/tauri";

export const projectsApi = {
  list(): Promise<Project[]> {
    return invokeCommand<Project[]>("list_projects");
  },

  record(path: string): Promise<Project> {
    return invokeCommand<Project>("record_project", { path });
  },

  delete(id: string): Promise<void> {
    return invokeCommand<void>("delete_project", { id });
  },

  reorder(projectIds: string[]): Promise<Project[]> {
    return invokeCommand<Project[]>("reorder_projects", { projectIds });
  },

  listGitBaseFolders(): Promise<GitBaseFolder[]> {
    return invokeCommand<GitBaseFolder[]>("list_git_base_folders");
  },

  recordGitBaseFolder(path: string): Promise<GitBaseFolder> {
    return invokeCommand<GitBaseFolder>("record_git_base_folder", { path });
  },

  removeGitBaseFolder(id: string): Promise<void> {
    return invokeCommand<void>("remove_git_base_folder", { id });
  },

  scanBaseFolder(path: string): Promise<ScanResult[]> {
    return invokeCommand<ScanResult[]>("scan_git_base_folder", { path });
  },

  scanGitBaseFolders(): Promise<ScanResult[]> {
    return invokeCommand<ScanResult[]>("scan_git_base_folders");
  },

  /** Replace the full set of Project custom skills directories. The backend
   *  normalizes and de-duplicates the list and rejects dirs that resolve to a
   *  fixed Agent project skills dir. Returns the updated Project. */
  setCustomSkillsDirs(projectId: string, dirs: string[]): Promise<Project> {
    return invokeCommand<Project>("set_project_custom_skills_dirs", { projectId, dirs });
  },

  /** Replace the full set of Project extra prompt files. The backend normalizes
   *  and de-duplicates the list and rejects entries whose filename does not match
   *  an Agent `projectPromptFile` glob (AGENTS*.md / CLAUDE*.md). Returns the
   *  updated Project. */
  setExtraPromptFiles(projectId: string, files: string[]): Promise<Project> {
    return invokeCommand<Project>("set_project_extra_prompt_files", { projectId, files });
  },

  /** Override the Project Session Directory. An empty string restores the default
   *  `__sessions` template. Session Directory stays single-valued by design.
   *  Returns the updated Project. */
  setSessionsDir(projectId: string, dir: string): Promise<Project> {
    return invokeCommand<Project>("set_project_sessions_dir", { projectId, dir });
  },
};
